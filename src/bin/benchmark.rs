use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Instant;

fn main() {
    println!("=== ScaryDB Performance Benchmark ===");
    
    // Connect to local ScaryDB server
    let stream = match TcpStream::connect("127.0.0.1:6379") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to ScaryDB: {}. Make sure the server is running in another terminal (e.g. cargo run -- server).", e);
            return;
        }
    };
    let mut write_stream = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);

    // Initialize database and bucket
    write_stream.write_all(b"CREATE DB bench_db;\n").unwrap();
    let mut resp = String::new();
    reader.read_line(&mut resp).unwrap();
    
    write_stream.write_all(b"USE bench_db;\n").unwrap();
    resp.clear();
    reader.read_line(&mut resp).unwrap();

    write_stream.write_all(b"CREATE BUCKET keys;\n").unwrap();
    resp.clear();
    reader.read_line(&mut resp).unwrap();

    // Set checkpoint interval high so it doesn't trigger during the benchmark
    write_stream.write_all(b"SET CONFIG storage.checkpoint_interval_ops 10000;\n").unwrap();
    resp.clear();
    reader.read_line(&mut resp).unwrap();

    println!("Starting 1000 sequential SET operations...");
    let start_set = Instant::now();
    for i in 0..1000 {
        let cmd = format!("SET keys k{} {};\n", i, i);
        write_stream.write_all(cmd.as_bytes()).unwrap();
        write_stream.flush().unwrap();
        resp.clear();
        reader.read_line(&mut resp).unwrap();
    }
    let duration_set = start_set.elapsed();
    println!("1000 SETs completed in: {:?}", duration_set);
    println!("Average SET latency: {:?}", duration_set / 1000);
    println!("SET operations per second: {:.2}", 1000.0 / duration_set.as_secs_f64());

    println!("Starting 1000 sequential GET operations...");
    let start_get = Instant::now();
    for i in 0..1000 {
        let cmd = format!("GET keys k{};\n", i);
        write_stream.write_all(cmd.as_bytes()).unwrap();
        write_stream.flush().unwrap();
        resp.clear();
        reader.read_line(&mut resp).unwrap();
    }
    let duration_get = start_get.elapsed();
    println!("1000 GETs completed in: {:?}", duration_get);
    println!("Average GET latency: {:?}", duration_get / 1000);
    println!("GET operations per second: {:.2}", 1000.0 / duration_get.as_secs_f64());
}
