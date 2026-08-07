use crate::engine::{DatabaseState, StorageEngine};
use crate::value::Value;
use crate::catalog::Catalog;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use flume::{bounded, Sender, Receiver};

pub const MAGIC_BYTES: &[u8; 4] = b"SCRY";

#[derive(Debug, Clone)]
pub enum LogOp {
    CreateDb { db_name: String },
    DropDb { db_name: String },
    CreateBucket { db_name: String, bucket_name: String },
    DropBucket { db_name: String, bucket_name: String },
    Set { db_name: String, bucket_name: String, key_name: String, value: Value },
    Del { db_name: String, bucket_name: String, key_name: String },
}

impl LogOp {
    #[inline]
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);
        match self {
            LogOp::CreateDb { db_name } => {
                buf.push(1);
                write_string(&mut buf, db_name);
            }
            LogOp::DropDb { db_name } => {
                buf.push(2);
                write_string(&mut buf, db_name);
            }
            LogOp::CreateBucket { db_name, bucket_name } => {
                buf.push(3);
                write_string(&mut buf, db_name);
                write_string(&mut buf, bucket_name);
            }
            LogOp::DropBucket { db_name, bucket_name } => {
                buf.push(4);
                write_string(&mut buf, db_name);
                write_string(&mut buf, bucket_name);
            }
            LogOp::Set { db_name, bucket_name, key_name, value } => {
                buf.push(5);
                write_string(&mut buf, db_name);
                write_string(&mut buf, bucket_name);
                write_string(&mut buf, key_name);
                write_value(&mut buf, value);
            }
            LogOp::Del { db_name, bucket_name, key_name } => {
                buf.push(6);
                write_string(&mut buf, db_name);
                write_string(&mut buf, bucket_name);
                write_string(&mut buf, key_name);
            }
        }
        buf
    }

    #[inline]
    pub fn deserialize(data: &[u8]) -> Result<Self, String> {
        let mut cursor = 0;
        if data.is_empty() {
            return Err("Empty log data".to_string());
        }
        let op_code = data[cursor];
        cursor += 1;

        match op_code {
            1 => {
                let db_name = read_string(&mut cursor, data)?;
                Ok(LogOp::CreateDb { db_name })
            }
            2 => {
                let db_name = read_string(&mut cursor, data)?;
                Ok(LogOp::DropDb { db_name })
            }
            3 => {
                let db_name = read_string(&mut cursor, data)?;
                let bucket_name = read_string(&mut cursor, data)?;
                Ok(LogOp::CreateBucket { db_name, bucket_name })
            }
            4 => {
                let db_name = read_string(&mut cursor, data)?;
                let bucket_name = read_string(&mut cursor, data)?;
                Ok(LogOp::DropBucket { db_name, bucket_name })
            }
            5 => {
                let db_name = read_string(&mut cursor, data)?;
                let bucket_name = read_string(&mut cursor, data)?;
                let key_name = read_string(&mut cursor, data)?;
                let value = read_value(&mut cursor, data)?;
                Ok(LogOp::Set { db_name, bucket_name, key_name, value })
            }
            6 => {
                let db_name = read_string(&mut cursor, data)?;
                let bucket_name = read_string(&mut cursor, data)?;
                let key_name = read_string(&mut cursor, data)?;
                Ok(LogOp::Del { db_name, bucket_name, key_name })
            }
            other => Err(format!("Unknown log op code: {}", other)),
        }
    }
}

// Optimized string serialization - no heap allocation for small strings
#[inline]
fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    buf.extend_from_slice(bytes);
}

#[inline]
fn read_string(cursor: &mut usize, data: &[u8]) -> Result<String, String> {
    if *cursor + 2 > data.len() {
        return Err("Buffer overflow reading string length".to_string());
    }
    let len = u16::from_be_bytes([data[*cursor], data[*cursor + 1]]) as usize;
    *cursor += 2;
    if *cursor + len > data.len() {
        return Err("Buffer overflow reading string body".to_string());
    }
    let s = std::str::from_utf8(&data[*cursor..*cursor + len])
        .map_err(|e| format!("Invalid UTF-8 string: {}", e))?;
    *cursor += len;
    Ok(s.to_string())
}

#[inline]
fn write_value(buf: &mut Vec<u8>, val: &Value) {
    match val {
        Value::String(s) => {
            buf.push(1);
            let bytes = s.as_bytes();
            buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            buf.extend_from_slice(bytes);
        }
        Value::Int(i) => {
            buf.push(2);
            buf.extend_from_slice(&i.to_be_bytes());
        }
        Value::Float(f) => {
            buf.push(3);
            buf.extend_from_slice(&f.to_be_bytes());
        }
        Value::Bool(b) => {
            buf.push(4);
            buf.push(if *b { 1 } else { 0 });
        }
    }
}

#[inline]
fn read_value(cursor: &mut usize, data: &[u8]) -> Result<Value, String> {
    if *cursor + 1 > data.len() {
        return Err("Buffer overflow reading value type".to_string());
    }
    let ty = data[*cursor];
    *cursor += 1;
    match ty {
        1 => {
            if *cursor + 4 > data.len() {
                return Err("Buffer overflow reading string value length".to_string());
            }
            let len = u32::from_be_bytes([
                data[*cursor],
                data[*cursor + 1],
                data[*cursor + 2],
                data[*cursor + 3],
            ]) as usize;
            *cursor += 4;
            if *cursor + len > data.len() {
                return Err("Buffer overflow reading string value body".to_string());
            }
            let s = std::str::from_utf8(&data[*cursor..*cursor + len])
                .map_err(|e| format!("Invalid UTF-8 string: {}", e))?;
            *cursor += len;
            Ok(Value::String(s.to_string()))
        }
        2 => {
            if *cursor + 8 > data.len() {
                return Err("Buffer overflow reading int value".to_string());
            }
            let i = i64::from_be_bytes([
                data[*cursor],
                data[*cursor + 1],
                data[*cursor + 2],
                data[*cursor + 3],
                data[*cursor + 4],
                data[*cursor + 5],
                data[*cursor + 6],
                data[*cursor + 7],
            ]);
            *cursor += 8;
            Ok(Value::Int(i))
        }
        3 => {
            if *cursor + 8 > data.len() {
                return Err("Buffer overflow reading float value".to_string());
            }
            let f = f64::from_be_bytes([
                data[*cursor],
                data[*cursor + 1],
                data[*cursor + 2],
                data[*cursor + 3],
                data[*cursor + 4],
                data[*cursor + 5],
                data[*cursor + 6],
                data[*cursor + 7],
            ]);
            *cursor += 8;
            Ok(Value::Float(f))
        }
        4 => {
            if *cursor >= data.len() {
                return Err("Buffer overflow reading bool value".to_string());
            }
            let b = data[*cursor] != 0;
            *cursor += 1;
            Ok(Value::Bool(b))
        }
        other => Err(format!("Unknown value type: {}", other)),
    }
}

// Background WAL writer message types
enum WriterMsg {
    Write(Vec<u8>),
    Flush(Sender<Result<(), String>>),
    Shutdown,
}

/// PersistenceManager with background WAL writer using flume channels.
/// Batches writes for high throughput, flushes periodically or on demand.
pub struct PersistenceManager {
    data_dir: PathBuf,
    catalog_path: PathBuf,
    log_file_path: PathBuf,
    log_file: Mutex<Option<File>>,
    tx_id: AtomicU64,
    
    // Background writer channel (bounded for backpressure)
    writer_tx: Sender<WriterMsg>,
    writer_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl PersistenceManager {
    pub fn new(data_dir: &Path) -> Self {
        let data_dir = data_dir.to_path_buf();
        let catalog_path = data_dir.join("catalog.db");
        let log_file_path = data_dir.join("operations.log");

        // Create directory if it doesn't exist
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir).ok();
        }

        // Bounded channel for backpressure under load
        let (writer_tx, writer_rx) = bounded(1000);
        
        // Spawn background writer thread
        let log_path = log_file_path.clone();
        let writer_handle = std::thread::spawn(move || {
            let mut file = None;
            let mut buffer = Vec::with_capacity(8192);
            
            while let Ok(msg) = writer_rx.recv() {
                match msg {
                    WriterMsg::Write(data) => {
                        buffer.extend_from_slice(&data);
                        // Auto-flush at 4KB to balance latency/throughput
                        if buffer.len() >= 4096 {
                            if let Some(ref mut f) = file {
                                let _ = f.write_all(&buffer);
                                let _ = f.flush();
                            } else {
                                // Lazy open
                                if let Ok(f) = OpenOptions::new().create(true).append(true).open(&log_path) {
                                    let _ = f.write_all(&buffer);
                                    let _ = f.flush();
                                    file = Some(f);
                                }
                            }
                            buffer.clear();
                        }
                    }
                    WriterMsg::Flush(resp_tx) => {
                        // Write remaining buffer
                        if !buffer.is_empty() {
                            if let Some(ref mut f) = file {
                                let _ = f.write_all(&buffer);
                                let _ = f.flush();
                            } else if let Ok(f) = OpenOptions::new().create(true).append(true).open(&log_path) {
                                let _ = f.write_all(&buffer);
                                let _ = f.flush();
                                file = Some(f);
                            }
                            buffer.clear();
                        } else if let Some(ref mut f) = file {
                            let _ = f.flush();
                        }
                        let _ = resp_tx.send(Ok(()));
                    }
                    WriterMsg::Shutdown => {
                        // Final flush
                        if !buffer.is_empty() {
                            if let Some(ref mut f) = file {
                                let _ = f.write_all(&buffer);
                                let _ = f.flush();
                            } else if let Ok(f) = OpenOptions::new().create(true).append(true).open(&log_path) {
                                let _ = f.write_all(&buffer);
                                let _ = f.flush();
                                file = Some(f);
                            }
                        }
                        break;
                    }
                }
            }
        });

        Self {
            data_dir,
            catalog_path,
            log_file_path,
            log_file: Mutex::new(None),
            tx_id: AtomicU64::new(0),
            writer_tx,
            writer_handle: Mutex::new(Some(writer_handle)),
        }
    }

    /// Append a log operation asynchronously (non-blocking for hot path).
    /// Returns tx_id immediately, actual write happens in background.
    #[inline]
    pub fn append_op(&self, op: &LogOp) -> Result<u64, String> {
        let tx_id = self.tx_id.fetch_add(1, Ordering::Relaxed);
        
        // Serialize operation
        let payload = op.serialize();
        
        // Build log entry: MAGIC(4) + payload_len(4) + tx_id(8) + payload
        let mut entry = Vec::with_capacity(16 + payload.len());
        entry.extend_from_slice(MAGIC_BYTES);
        entry.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        entry.extend_from_slice(&tx_id.to_be_bytes());
        entry.extend(payload);

        // Non-blocking send - apply backpressure if channel full
        self.writer_tx.send(WriterMsg::Write(entry))
            .map_err(|_| "WAL writer channel full, backpressure applied".to_string())?;
        
        Ok(tx_id)
    }

    /// Flush all pending writes and wait for completion (for checkpoints/shutdown).
    pub fn flush(&self) -> Result<(), String> {
        let (resp_tx, resp_rx) = bounded(1);
        self.writer_tx.send(WriterMsg::Flush(resp_tx))
            .map_err(|_| "WAL writer channel closed".to_string())?;
        resp_rx.recv()
            .map_err(|_| "WAL writer response channel closed".to_string())?
    }

    /// Gracefully shutdown the background writer.
    pub fn shutdown(&self) -> Result<(), String> {
        // Send shutdown signal
        self.writer_tx.send(WriterMsg::Shutdown)
            .map_err(|_| "WAL writer channel closed".to_string())?;
        
        // Wait for thread to finish
        if let Some(handle) = self.writer_handle.lock().take() {
            handle.join().ok();
        }
        Ok(())
    }

    /// Read and decode all operations in operations.log.
    pub fn read_log(&self) -> Result<Vec<(u64, LogOp)>, String> {
        if !self.log_file_path.exists() {
            return Ok(Vec::new());
        }

        let mut file = File::open(&self.log_file_path)
            .map_err(|e| format!("Failed to open log file for reading: {}", e))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| format!("Failed to read log file contents: {}", e))?;

        let mut cursor = 0;
        let mut ops = Vec::new();

        while cursor < data.len() {
            if cursor + 16 > data.len() {
                return Err("Log file truncated or corrupted (header too short)".to_string());
            }

            let magic = &data[cursor..cursor + 4];
            if magic != MAGIC_BYTES {
                return Err(format!(
                    "Invalid magic bytes in log at offset {}. Found {:?}",
                    cursor, magic
                ));
            }
            cursor += 4;

            let payload_len = u32::from_be_bytes([
                data[cursor],
                data[cursor + 1],
                data[cursor + 2],
                data[cursor + 3],
            ]) as usize;
            cursor += 4;

            let tx_id = u64::from_be_bytes([
                data[cursor],
                data[cursor + 1],
                data[cursor + 2],
                data[cursor + 3],
                data[cursor + 4],
                data[cursor + 5],
                data[cursor + 6],
                data[cursor + 7],
            ]);
            cursor += 8;

            if cursor + payload_len > data.len() {
                return Err(format!(
                    "Log file truncated. Expected payload size {}, available {}",
                    payload_len,
                    data.len() - cursor
                ));
            }

            let op = LogOp::deserialize(&data[cursor..cursor + payload_len])
                .map_err(|e| format!("Failed to deserialize log op at tx_id {}: {}", tx_id, e))?;
            cursor += payload_len;

            ops.push((tx_id, op));
        }

        Ok(ops)
    }

    /// Write active database engine state to files and clear the operations log.
    pub fn checkpoint(&self, engine: &StorageEngine) -> Result<(), String> {
        // Flush any pending WAL writes first
        self.flush()?;

        // 1. Save global catalog
        let catalog_json = serde_json::to_string_pretty(&engine.global_catalog)
            .map_err(|e| format!("Failed to serialize catalog: {}", e))?;
        fs::write(&self.catalog_path, catalog_json)
            .map_err(|e| format!("Failed to write catalog file: {}", e))?;

        // 2. Save each database state as an individual file
        for (db_id, db_state) in &engine.databases {
            if let Some(db_name) = engine.global_catalog.db_id_to_name.get(db_id) {
                let db_file_path = self.data_dir.join(format!("{}.db", db_name));
                let db_json = serde_json::to_string_pretty(db_state)
                    .map_err(|e| format!("Failed to serialize db state: {}", e))?;
                fs::write(&db_file_path, db_json)
                    .map_err(|e| format!("Failed to write db file {}: {}", db_name, e))?;
            }
        }

        // 3. Truncate operations log
        if self.log_file_path.exists() {
            let file = File::create(&self.log_file_path)
                .map_err(|e| format!("Failed to truncate log file: {}", e))?;
            file.set_len(0)
                .map_err(|e| format!("Failed to resize log file: {}", e))?;
        }

        Ok(())
    }

    /// Restore the database state from catalog.db, <db>.db files, and replay operations.log.
    pub fn restore(&self, engine: &mut StorageEngine) -> Result<(), String> {
        // 1. Load catalog if it exists
        if self.catalog_path.exists() {
            let catalog_json = fs::read_to_string(&self.catalog_path)
                .map_err(|e| format!("Failed to read catalog file: {}", e))?;
            let catalog: Catalog = if catalog_json.trim().is_empty() {
                Catalog::default()
            } else {
                serde_json::from_str(&catalog_json)
                    .map_err(|e| format!("Failed to parse catalog file: {}", e))?
            };
            engine.global_catalog = catalog;

            // 2. Load individual database files
            for (&db_id, db_name) in &engine.global_catalog.db_id_to_name {
                let db_file_path = self.data_dir.join(format!("{}.db", db_name));
                if db_file_path.exists() {
                    let db_json = fs::read_to_string(&db_file_path)
                        .map_err(|e| format!("Failed to read database file {}: {}", db_name, e))?;
                    let db_state: DatabaseState = if db_json.trim().is_empty() {
                        DatabaseState::default()
                    } else {
                        serde_json::from_str(&db_json)
                            .map_err(|e| format!("Failed to parse database file {}: {}", db_name, e))?
                    };
                    engine.databases.insert(db_id, db_state);
                } else {
                    // Initialize if missing
                    engine.databases.insert(db_id, DatabaseState::default());
                }
            }
        }

        // 3. Replay operations from WAL
        let logs = self.read_log()?;
        for (tx_id, log_op) in logs {
            match log_op {
                LogOp::CreateDb { db_name } => {
                    let _ = engine.create_db(&db_name);
                }
                LogOp::DropDb { db_name } => {
                    let _ = engine.drop_db(&db_name);
                }
                LogOp::CreateBucket { db_name, bucket_name } => {
                    if let Some(db_id) = engine.global_catalog.get_db_id(&db_name) {
                        let _ = engine.create_bucket(db_id, &bucket_name);
                    }
                }
                LogOp::DropBucket { db_name, bucket_name } => {
                    if let Some(db_id) = engine.global_catalog.get_db_id(&db_name) {
                        let _ = engine.drop_bucket(db_id, &bucket_name);
                    }
                }
                LogOp::Set { db_name, bucket_name, key_name, value } => {
                    if let Some(db_id) = engine.global_catalog.get_db_id(&db_name) {
                        let _ = engine.set_key(db_id, &bucket_name, &key_name, value);
                    }
                }
                LogOp::Del { db_name, bucket_name, key_name } => {
                    if let Some(db_id) = engine.global_catalog.get_db_id(&db_name) {
                        let _ = engine.del_key(db_id, &bucket_name, &key_name);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Drop for PersistenceManager {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}