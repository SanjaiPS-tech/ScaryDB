mod catalog;
mod config;
mod engine;
mod parser;
mod persistence;
mod value;
mod worker;

use config::Config;
use parser::parse_command;
use persistence::PersistenceManager;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use worker::{DatabaseSystem, Request, Response, WorkerPool};

pub static QUIET: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize, Debug)]
struct WireResponse {
    status: String,
    message: String,
    active_db: Option<String>,
}

const CONFIG_PATH: &str = "config.json";

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = if args.len() > 1 {
        args[1].to_lowercase()
    } else {
        "server".to_string()
    };

    match mode.as_str() {
        "standalone" | "--standalone" => run_standalone(),
        "server" | "--server" => run_server(),
        "client" | "--client" => run_client(false),
        "log-read" | "--log-read" => {
            if args.len() < 3 {
                println!("Usage: scarydb log-read <path_to_operations.log>");
                return;
            }
            run_log_reader(&args[2]);
        }
        _ => {
            println!("Unknown mode: {}. Use 'standalone', 'server', 'client', or 'log-read'.", mode);
        }
    }
}

// --- DROP IMPLEMENTATION FOR DB SYSTEM FOR GRACEFUL SHUTDOWN ---

impl Drop for DatabaseSystem {
    fn drop(&mut self) {
        if !crate::QUIET.load(Ordering::Relaxed) {
            println!("Shutting down ScaryDB... Performing final database checkpoint.");
        }
        if let Err(e) = self.persistence.checkpoint(&self.engine) {
            eprintln!("Failed to save database state on shutdown: {}", e);
        } else {
            if !crate::QUIET.load(Ordering::Relaxed) {
                println!("Final checkpoint written successfully. Goodbye!");
            }
        }
    }
}

// --- STANDALONE MODE ---

fn run_standalone() {
    QUIET.store(true, Ordering::Relaxed);
    println!("=== ScaryDB Standalone Mode (Server + Client) ===");
    let config = match Config::load_or_create(CONFIG_PATH) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Fatal: Failed to load configuration: {}", e);
            return;
        }
    };

    let mut system = DatabaseSystem::new(config.clone(), CONFIG_PATH);
    if let Err(e) = system.init_and_restore() {
        eprintln!("Fatal: Storage initialization failed: {}", e);
        return;
    }

    let system_arc = Arc::new(Mutex::new(system));
    let (request_tx, request_rx) = mpsc::channel::<Request>();
    let request_rx_arc = Arc::new(Mutex::new(request_rx));

    let _worker_pool = WorkerPool::new(config.server.workers, request_rx_arc, Arc::clone(&system_arc));

    let address = format!("{}:{}", config.network.host, config.network.port);
    let listener = match TcpListener::bind(&address) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Fatal: Failed to bind to TCP address {}: {}", address, e);
            return;
        }
    };
    
    let sys_for_server = Arc::clone(&system_arc);
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    let tx = request_tx.clone();
                    let sys = Arc::clone(&sys_for_server);
                    thread::spawn(move || {
                        handle_client_connection(s, tx, sys);
                    });
                }
                Err(_) => break,
            }
        }
    });

    thread::sleep(std::time::Duration::from_millis(100)); // Give server a moment to bind
    
    // Now run client directly in the main thread
    run_client(true);
    
    let quiet = QUIET.load(Ordering::Relaxed);
    if !quiet {
        println!("Standalone client exited. Saving DB state...");
    }
    let mut sys = system_arc.lock().unwrap();
    let sys_ref = &mut *sys;
    if let Err(e) = sys_ref.persistence.checkpoint(&sys_ref.engine) {
        eprintln!("Failed to save database state on shutdown: {}", e);
    } else {
        if !quiet {
            println!("Final checkpoint written successfully. Goodbye!");
        }
    }
    std::process::exit(0);
}

// --- SERVER MODE ---

fn run_server() {
    println!("=== ScaryDB Database Server ===");
    let config = match Config::load_or_create(CONFIG_PATH) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Fatal: Failed to load configuration: {}", e);
            return;
        }
    };

    println!("Loaded config: host={}, port={}, workers={}", config.network.host, config.network.port, config.server.workers);

    let mut system = DatabaseSystem::new(config.clone(), CONFIG_PATH);
    if let Err(e) = system.init_and_restore() {
        eprintln!("Fatal: Storage initialization failed: {}", e);
        return;
    }
    println!("Database storage initialized and restored.");

    let system_arc = Arc::new(Mutex::new(system));
    let (request_tx, request_rx) = mpsc::channel::<Request>();
    let request_rx_arc = Arc::new(Mutex::new(request_rx));

    // Spawn workers
    let _worker_pool = WorkerPool::new(config.server.workers, request_rx_arc, Arc::clone(&system_arc));

    let address = format!("{}:{}", config.network.host, config.network.port);
    let listener = match TcpListener::bind(&address) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Fatal: Failed to bind to TCP address {}: {}", address, e);
            return;
        }
    };
    println!("ScaryDB Server listening on {}...", address);

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let tx = request_tx.clone();
                let sys = Arc::clone(&system_arc);
                thread::spawn(move || {
                    handle_client_connection(s, tx, sys);
                });
            }
            Err(e) => {
                eprintln!("Connection accept failed: {}", e);
            }
        }
    }
}

fn handle_client_connection(
    stream: TcpStream,
    request_tx: Sender<Request>,
    system: Arc<Mutex<DatabaseSystem>>,
) {
    let mut write_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to clone stream: {}", e);
            return;
        }
    };
    let peer_addr = write_stream.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "unknown".to_string());
    
    let quiet = QUIET.load(Ordering::Relaxed);
    if !quiet {
        println!("Client connected: {}", peer_addr);
    }

    let reader = BufReader::new(stream);
    let mut db_context: Option<u32> = None;

    let (response_tx, response_rx) = mpsc::channel::<Response>();

    for line in reader.lines() {
        let raw_line = match line {
            Ok(l) => l,
            Err(_) => break, // Client disconnected
        };

        let cmd_text = raw_line.trim();
        if cmd_text.is_empty() {
            continue;
        }

        if !quiet {
            println!("[Server] Received line: '{}'", cmd_text);
        }

        // Check if exit command
        if cmd_text.to_uppercase() == "EXIT" || cmd_text.to_uppercase() == "QUIT" {
            if !quiet {
                println!("[Server] Client requested disconnect.");
            }
            break;
        }

        // Parse command
        let command = match parse_command(cmd_text) {
            Ok(cmd) => {
                if !quiet {
                    println!("[Server] Parsed command: {:?}", cmd);
                }
                cmd
            }
            Err(err_msg) => {
                if !quiet {
                    println!("[Server] Parse error: {}", err_msg);
                }
                let wire_res = WireResponse {
                    status: "err".to_string(),
                    message: err_msg,
                    active_db: get_active_db_name(&system, db_context),
                };
                let _ = send_wire_response(&mut write_stream, &wire_res);
                continue;
            }
        };

        // Submit request to worker queue
        let req = Request {
            command,
            db_context,
            response_tx: response_tx.clone(),
        };

        if !quiet {
            println!("[Server] Enqueuing request to worker pool...");
        }
        if request_tx.send(req).is_err() {
            if !quiet {
                eprintln!("[Server] Failed to route request: Server worker queue closed.");
            }
            break;
        }

        // Wait for response
        if !quiet {
            println!("[Server] Waiting for worker response...");
        }
        let res = match response_rx.recv() {
            Ok(r) => r,
            Err(_) => {
                if !quiet {
                    eprintln!("[Server] Response channel closed unexpectedly.");
                }
                break;
            }
        };
        if !quiet {
            println!("[Server] Worker response received: {:?}", res.result);
        }

        // Update local session db_context
        db_context = res.updated_db_context;

        // Formulate wire response
        let wire_res = match res.result {
            Ok(msg) => WireResponse {
                status: "ok".to_string(),
                message: msg,
                active_db: get_active_db_name(&system, db_context),
            },
            Err(msg) => WireResponse {
                status: "err".to_string(),
                message: msg,
                active_db: get_active_db_name(&system, db_context),
            },
        };

        if !quiet {
            println!("[Server] Sending response to client...");
        }
        if send_wire_response(&mut write_stream, &wire_res).is_err() {
            if !quiet {
                println!("[Server] Failed to send response to client.");
            }
            break;
        }
        if !quiet {
            println!("[Server] Response successfully sent.");
        }
    }

    if !quiet {
        println!("Client disconnected: {}", peer_addr);
    }
}

fn get_active_db_name(system: &Arc<Mutex<DatabaseSystem>>, db_id: Option<u32>) -> Option<String> {
    db_id.and_then(|id| {
        let sys = system.lock().unwrap();
        sys.engine.global_catalog.db_id_to_name.get(&id).cloned()
    })
}

fn send_wire_response(stream: &mut TcpStream, res: &WireResponse) -> io::Result<()> {
    let mut json_bytes = serde_json::to_vec(res).unwrap();
    json_bytes.push(b'\n');
    stream.write_all(&json_bytes)?;
    stream.flush()?;
    Ok(())
}

// --- CLIENT MODE (REPL) ---

fn run_client(is_standalone: bool) {
    println!("=== ScaryDB CLI Client ===");
    let config = match Config::load_or_create(CONFIG_PATH) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            return;
        }
    };

    let address = format!("{}:{}", config.network.host, config.network.port);
    println!("Connecting to {}...", address);
    let stream = match TcpStream::connect(&address) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Fatal: Connection failed: {}", e);
            return;
        }
    };
    println!("Connected successfully! Type 'HELP' for instructions or 'EXIT' to quit.\n");

    let mut write_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Fatal: stream clone failed: {}", e);
            return;
        }
    };
    let mut stdin_reader = io::BufReader::new(io::stdin());
    let mut server_reader = io::BufReader::new(stream);
    let mut active_db: Option<String> = None;

    loop {
        // Render Prompt
        match &active_db {
            Some(db) => print!("scarydb ({})> ", db),
            None => print!("scarydb> "),
        }
        let _ = io::stdout().flush();

        let mut line = String::new();
        if stdin_reader.read_line(&mut line).is_err() {
            break;
        }

        let cmd = line.trim();
        if cmd.is_empty() {
            continue;
        }

        if cmd.to_uppercase() == "EXIT" || cmd.to_uppercase() == "QUIT" {
            break;
        }

        let start_time = std::time::Instant::now();

        // Send to server
        if write_stream.write_all(line.as_bytes()).is_err() || write_stream.write_all(b"\n").is_err() || write_stream.flush().is_err() {
            println!("Connection to server lost.");
            break;
        }

        // Read response
        let mut resp_line = String::new();
        if server_reader.read_line(&mut resp_line).is_err() {
            println!("Connection to server lost while reading response.");
            break;
        }

        let duration = start_time.elapsed();

        if resp_line.is_empty() {
            println!("Empty response from server.");
            break;
        }

        match serde_json::from_str::<WireResponse>(&resp_line) {
            Ok(wire_res) => {
                active_db = wire_res.active_db;
                if wire_res.status == "ok" {
                    println!("{}", wire_res.message);
                } else {
                    println!("ERROR: {}", wire_res.message);
                }

                if is_standalone {
                    if duration.as_secs() > 0 {
                        println!("({:.2}s)", duration.as_secs_f64());
                    } else {
                        println!("({:.2}ms)", duration.as_secs_f64() * 1000.0);
                    }
                }
            }
            Err(e) => {
                println!("Failed to parse server response: {}. Raw: {}", e, resp_line);
            }
        }
        println!(); // new line
    }
}

// --- LOG READER MODE ---

fn run_log_reader(path: &str) {
    println!("=== ScaryDB Log Reader ===");
    println!("Opening operations log: {}", path);
    let manager = PersistenceManager::new(
        Path::new(path)
            .parent()
            .unwrap_or_else(|| Path::new(".")),
    );
    match manager.read_log() {
        Ok(logs) => {
            println!("Successfully read {} transaction(s):", logs.len());
            for (tx_id, op) in logs {
                match op {
                    persistence::LogOp::CreateDb { db_name } => {
                        println!("[Tx {}] CREATE_DB: name='{}'", tx_id, db_name);
                    }
                    persistence::LogOp::DropDb { db_name } => {
                        println!("[Tx {}] DROP_DB: name='{}'", tx_id, db_name);
                    }
                    persistence::LogOp::CreateBucket { db_name, bucket_name } => {
                        println!("[Tx {}] CREATE_BUCKET: db='{}', bucket='{}'", tx_id, db_name, bucket_name);
                    }
                    persistence::LogOp::DropBucket { db_name, bucket_name } => {
                        println!("[Tx {}] DROP_BUCKET: db='{}', bucket='{}'", tx_id, db_name, bucket_name);
                    }
                    persistence::LogOp::Set { db_name, bucket_name, key_name, value } => {
                        println!(
                            "[Tx {}] SET: db='{}', bucket='{}', key='{}', type='{:?}', value={}",
                            tx_id, db_name, bucket_name, key_name, value, value
                        );
                    }
                    persistence::LogOp::Del { db_name, bucket_name, key_name } => {
                        println!("[Tx {}] DEL: db='{}', bucket='{}', key='{}'", tx_id, db_name, bucket_name, key_name);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error reading operations log file: {}", e);
        }
    }
}
