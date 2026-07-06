use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{PathBuf, Path};
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

fn find_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn get_binary_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("release");
    path.push("scarydb");
    path
}

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_test_dirs() -> (PathBuf, PathBuf) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let test_id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let test_base = base.join("target").join("test_data").join(format!("test_{}_{}", std::process::id(), test_id));
    let data_dir = test_base.join("data");
    let config_dir = test_base.join("config");
    fs::create_dir_all(&data_dir).unwrap();
    fs::create_dir_all(&config_dir).unwrap();
    (data_dir, config_dir)
}

fn write_test_config(config_dir: &Path, port: u16, data_dir: &Path) -> PathBuf {
    let config = format!(
        r#"{{
  "server": {{
    "workers": 1
  }},
  "storage": {{
    "data_dir": "{}",
    "checkpoint_interval_ops": 10000
  }},
  "memory": {{
    "max_memory_kb": 0
  }},
  "network": {{
    "host": "127.0.0.1",
    "port": {}
  }},
  "metadata": {{
    "version": "0.1.0",
    "startup_time": "2026-01-01T00:00:00Z"
  }}
}}"#,
        data_dir.display(), port);
    
    let config_path = config_dir.join("config.json");
    fs::write(&config_path, config).unwrap();
    config_path
}

fn start_test_server(port: u16, config_path: &Path) -> std::process::Child {
    let binary = get_binary_path();
    let mut cmd = Command::new(binary);
    cmd.args(["server"])
        .env("SCARYDB_TEST_PORT", port.to_string())
        .env("SCARYDB_CONFIG_PATH", config_path);
    
    let child = cmd.spawn().expect("Failed to start server");
    thread::sleep(Duration::from_millis(500));
    child
}

fn connect(port: u16) -> (TcpStream, BufReader<TcpStream>, TcpStream) {
    let addr = format!("127.0.0.1:{}", port);
    let stream = TcpStream::connect(&addr).expect(&format!("Failed to connect to {}", addr));
    let reader = BufReader::new(stream.try_clone().unwrap());
    let writer = stream.try_clone().unwrap();
    (stream, reader, writer)
}

fn send_cmd(writer: &mut TcpStream, reader: &mut BufReader<TcpStream>, cmd: &str) -> String {
    writer.write_all(cmd.as_bytes()).unwrap();
    writer.flush().unwrap();
    let mut resp = String::new();
    reader.read_line(&mut resp).unwrap();
    resp.trim().to_string()
}

fn run_test<F>(f: F) 
where 
    F: FnOnce(&mut BufReader<TcpStream>, &mut TcpStream, &Path),
{
    let port = find_free_port();
    let (data_dir, config_dir) = create_test_dirs();
    let config_path = write_test_config(&config_dir, port, &data_dir);
    let _server = start_test_server(port, &config_path);
    let (_stream, mut reader, mut writer) = connect(port);
    f(&mut reader, &mut writer, &data_dir);
    // Cleanup
    let _ = fs::remove_dir_all(config_dir.parent().unwrap());
}

#[test]
fn test_create_db_and_use() {
    run_test(|reader, writer, _| {
        let _resp = send_cmd(writer, reader, "CREATE DB test_db;\n");
        let resp = send_cmd(writer, reader, "USE test_db;\n");
        assert!(resp.contains("ok") || resp.contains("Switched"), "USE failed: {}", resp);
    });
}

#[test]
fn test_create_bucket_and_set_get() {
    run_test(|reader, writer, _| {
        send_cmd(writer, reader, "CREATE DB test_db;\n");
        send_cmd(writer, reader, "USE test_db;\n");
        send_cmd(writer, reader, "CREATE BUCKET users;\n");
        
        let resp = send_cmd(writer, reader, "SET users u1 \"Alice\";\n");
        assert!(resp.contains("ok") || resp.contains("set"), "SET failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "GET users u1;\n");
        assert!(resp.contains("Alice"), "GET failed: {}", resp);
    });
}

#[test]
fn test_batch_set_pipeline() {
    run_test(|reader, writer, _| {
        send_cmd(writer, reader, "CREATE DB test_db;\n");
        send_cmd(writer, reader, "USE test_db;\n");
        send_cmd(writer, reader, "CREATE BUCKET data;\n");
        
        // Send multiple SETs pipelined
        for i in 0..100 {
            let cmd = format!("SET data key{} {};\n", i, i);
            writer.write_all(cmd.as_bytes()).unwrap();
        }
        writer.flush().unwrap();
        
        // Read all responses
        for _ in 0..100 {
            let mut resp = String::new();
            reader.read_line(&mut resp).unwrap();
            assert!(resp.contains("ok") || resp.contains("set"), "Batch SET failed: {}", resp);
        }
        
        // Verify all GETs
        for i in 0..100 {
            let cmd = format!("GET data key{};\n", i);
            writer.write_all(cmd.as_bytes()).unwrap();
        }
        writer.flush().unwrap();
        
        for i in 0..100 {
            let mut resp = String::new();
            reader.read_line(&mut resp).unwrap();
            assert!(resp.contains(&i.to_string()), "Batch GET failed for key{}: {}", i, resp);
        }
    });
}

#[test]
fn test_delete() {
    run_test(|reader, writer, _| {
        send_cmd(writer, reader, "CREATE DB test_db;\n");
        send_cmd(writer, reader, "USE test_db;\n");
        send_cmd(writer, reader, "CREATE BUCKET users;\n");
        send_cmd(writer, reader, "SET users u1 \"Alice\";\n");
        
        let resp = send_cmd(writer, reader, "DEL users u1;\n");
        assert!(resp.contains("ok") || resp.contains("Deleted"), "DEL failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "GET users u1;\n");
        assert!(resp.contains("(nil)"), "GET after DEL should return nil: {}", resp);
    });
}

#[test]
fn test_exists() {
    run_test(|reader, writer, _| {
        send_cmd(writer, reader, "CREATE DB test_db;\n");
        send_cmd(writer, reader, "USE test_db;\n");
        send_cmd(writer, reader, "CREATE BUCKET users;\n");
        send_cmd(writer, reader, "SET users u1 \"Alice\";\n");
        
        let resp = send_cmd(writer, reader, "EXISTS users u1;\n");
        assert!(resp.contains("true"), "EXISTS should be true: {}", resp);
        
        let resp = send_cmd(writer, reader, "EXISTS users u2;\n");
        assert!(resp.contains("false"), "EXISTS should be false: {}", resp);
    });
}

#[test]
fn test_list_keys_and_count() {
    run_test(|reader, writer, _| {
        send_cmd(writer, reader, "CREATE DB test_db;\n");
        send_cmd(writer, reader, "USE test_db;\n");
        send_cmd(writer, reader, "CREATE BUCKET users;\n");
        send_cmd(writer, reader, "SET users u1 \"Alice\";\n");
        send_cmd(writer, reader, "SET users u2 \"Bob\";\n");
        
        let resp = send_cmd(writer, reader, "LIST users;\n");
        assert!(resp.contains("u1") && resp.contains("u2"), "LIST failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "COUNT users;\n");
        assert!(resp.contains("2"), "COUNT failed: {}", resp);
    });
}

#[test]
fn test_drop_bucket_and_db() {
    run_test(|reader, writer, _| {
        send_cmd(writer, reader, "CREATE DB test_db;\n");
        send_cmd(writer, reader, "USE test_db;\n");
        send_cmd(writer, reader, "CREATE BUCKET users;\n");
        
        let resp = send_cmd(writer, reader, "DROP BUCKET users;\n");
        assert!(resp.contains("ok") || resp.contains("dropped"), "DROP BUCKET failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "DROP DB test_db;\n");
        assert!(resp.contains("ok") || resp.contains("dropped"), "DROP DB failed: {}", resp);
    });
}

#[test]
fn test_system_commands() {
    run_test(|reader, writer, _| {
        let resp = send_cmd(writer, reader, "PING;\n");
        assert!(resp.contains("BOINK"), "PING failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "INFO;\n");
        assert!(resp.contains("ScaryDB") || resp.contains("v"), "INFO failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "STATS;\n");
        assert!(resp.contains("Databases"), "STATS failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "VERSION;\n");
        assert!(resp.contains("ScaryDB"), "VERSION failed: {}", resp);
    });
}

#[test]
fn test_config_commands() {
    run_test(|reader, writer, _| {
        let resp = send_cmd(writer, reader, "LIST CONFIG;\n");
        assert!(resp.contains("workers"), "LIST CONFIG failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "GET CONFIG server.workers;\n");
        assert!(resp.contains("1") || resp.contains("workers"), "GET CONFIG failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "SET CONFIG server.workers 2;\n");
        assert!(resp.contains("ok") || resp.contains("updated"), "SET CONFIG failed: {}", resp);
    });
}

#[test]
fn test_type_tags() {
    run_test(|reader, writer, _| {
        send_cmd(writer, reader, "CREATE DB test_db;\n");
        send_cmd(writer, reader, "USE test_db;\n");
        send_cmd(writer, reader, "CREATE BUCKET data;\n");
        
        send_cmd(writer, reader, "SET data i [INT] 42;\n");
        send_cmd(writer, reader, "SET data f [FLOAT] 3.14;\n");
        send_cmd(writer, reader, "SET data b [BOOL] true;\n");
        send_cmd(writer, reader, "SET data s [STRING] hello;\n");
        
        let resp = send_cmd(writer, reader, "GET data i;\n");
        assert!(resp.contains("42"), "INT failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "GET data f;\n");
        assert!(resp.contains("3.14"), "FLOAT failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "GET data b;\n");
        assert!(resp.contains("true"), "BOOL failed: {}", resp);
        
        let resp = send_cmd(writer, reader, "GET data s;\n");
        assert!(resp.contains("hello"), "STRING failed: {}", resp);
    });
}