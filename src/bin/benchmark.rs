use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Instant;

fn main() {
    println!("=== ScaryDB Performance Benchmark ===");

    // Allow configurable host/port via environment or use defaults matching config.json
    let host = env::var("SCARYDB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("SCARYDB_PORT")
        .unwrap_or_else(|_| "6379".to_string())
        .parse::<u16>()
        .expect("Invalid port");
    let address = format!("{}:{}", host, port);

    println!("Connecting to {}...", address);

    // Connect to local ScaryDB server
    let stream = match TcpStream::connect(&address) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Failed to connect to ScaryDB at {}: {}. Make sure the server is running in another terminal (e.g. cargo run -- server).",
                address, e
            );
            return;
        }
    };
    let mut write_stream = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);

    // Initialize database and bucket
    send_and_read(&mut write_stream, &mut reader, "CREATE DB bench_db;\n");
    send_and_read(&mut write_stream, &mut reader, "USE bench_db;\n");
    send_and_read(&mut write_stream, &mut reader, "CREATE BUCKET keys;\n");

    // Set checkpoint interval high so it doesn't trigger during the benchmark
    send_and_read(&mut write_stream, &mut reader, "SET CONFIG storage.checkpoint_interval_ops 10000;\n");

    println!("\n--- Benchmark: 1000 pipelined SET operations ---");
    // PIPELINE: Send all requests without waiting for responses
    let start_set = Instant::now();
    for i in 0..1000 {
        let cmd = format!("SET keys k{} {};\n", i, i);
        write_stream.write_all(cmd.as_bytes()).unwrap();
    }
    write_stream.flush().unwrap();

    // Then read all responses
    for _ in 0..1000 {
        let mut resp = String::new();
        reader.read_line(&mut resp).unwrap();
        // Optionally validate response
        // assert!(resp.contains("ok") || resp.contains("err"), "Unexpected response: {}", resp);
    }
    let duration_set = start_set.elapsed();

    let set_ops = 1000.0;
    let set_secs = duration_set.as_secs_f64();
    println!("1000 SETs completed in: {:?}", duration_set);
    println!("Average SET latency: {:?}", duration_set / 1000);
    println!("SET throughput: {:.2} ops/sec", set_ops / set_secs);

    println!("\n--- Benchmark: 1000 pipelined GET operations ---");
    let start_get = Instant::now();
    for i in 0..1000 {
        let cmd = format!("GET keys k{};\n", i);
        write_stream.write_all(cmd.as_bytes()).unwrap();
    }
    write_stream.flush().unwrap();

    // Then read all responses
    for _ in 0..1000 {
        let mut resp = String::new();
        reader.read_line(&mut resp).unwrap();
    }
    let duration_get = start_get.elapsed();

    let get_ops = 1000.0;
    let get_secs = duration_get.as_secs_f64();
    println!("1000 GETs completed in: {:?}", duration_get);
    println!("Average GET latency: {:?}", duration_get / 1000);
    println!("GET throughput: {:.2} ops/sec", get_ops / get_secs);

    println!("\n=== Summary ===");
    println!("SET: {:.2} ops/sec (avg {:.2} µs)", set_ops / set_secs, set_secs * 1_000_000.0 / set_ops);
    println!("GET: {:.2} ops/sec (avg {:.2} µs)", get_ops / get_secs, get_secs * 1_000_000.0 / get_ops);
}

fn send_and_read(write_stream: &mut TcpStream, reader: &mut BufReader<TcpStream>, cmd: &str) {
    write_stream.write_all(cmd.as_bytes()).unwrap();
    write_stream.flush().unwrap();
    let mut resp = String::new();
    reader.read_line(&mut resp).unwrap();
}