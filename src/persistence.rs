use crate::engine::{DatabaseState, StorageEngine};
use crate::value::Value;
use crate::catalog::Catalog;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::time::Duration;

pub const MAGIC_BYTES: &[u8; 4] = b"SCRY";

#[derive(Debug, Clone)]
pub enum LogOp {
    CreateDb { db_name: String },
    DropDb { db_name: String },
    CreateBucket { db_name: String, bucket_name: String },
    DropBucket { db_name: String, bucket_name: String },
    Set { db_name: String, bucket_name: String, key_name: String, value: Value },
    Del { db_name: String, bucket_name: String, key_name: String },
    Batch(Vec<LogOp>),
}

impl LogOp {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            LogOp::CreateDb { db_name } => {
                buf.push(1); // op code
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
            LogOp::Batch(ops) => {
                buf.push(7);
                buf.extend_from_slice(&(ops.len() as u32).to_be_bytes());
                for op in ops {
                    let serialized = op.serialize();
                    buf.extend_from_slice(&(serialized.len() as u32).to_be_bytes());
                    buf.extend(serialized);
                }
            }
        }
        buf
    }

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
            7 => {
                if cursor + 4 > data.len() {
                    return Err("Buffer overflow reading batch count".to_string());
                }
                let count = u32::from_be_bytes([
                    data[cursor],
                    data[cursor + 1],
                    data[cursor + 2],
                    data[cursor + 3],
                ]) as usize;
                cursor += 4;
                
                let mut ops = Vec::with_capacity(count);
                for _ in 0..count {
                    if cursor + 4 > data.len() {
                        return Err("Buffer overflow reading batch op length".to_string());
                    }
                    let op_len = u32::from_be_bytes([
                        data[cursor],
                        data[cursor + 1],
                        data[cursor + 2],
                        data[cursor + 3],
                    ]) as usize;
                    cursor += 4;
                    
                    if cursor + op_len > data.len() {
                        return Err("Buffer overflow reading batch op".to_string());
                    }
                    
                    let op = LogOp::deserialize(&data[cursor..cursor + op_len])?;
                    cursor += op_len;
                    ops.push(op);
                }
                Ok(LogOp::Batch(ops))
            }
            other => Err(format!("Unknown log op code: {}", other)),
        }
    }
}

// Helpers for reading/writing binary data to buffer
fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    buf.extend_from_slice(bytes);
}

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
                .map_err(|e| format!("Invalid UTF-8 string in value: {}", e))?;
            *cursor += len;
            Ok(Value::String(s.to_string()))
        }
        2 => {
            if *cursor + 8 > data.len() {
                return Err("Buffer overflow reading int value".to_string());
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&data[*cursor..*cursor + 8]);
            *cursor += 8;
            Ok(Value::Int(i64::from_be_bytes(bytes)))
        }
        3 => {
            if *cursor + 8 > data.len() {
                return Err("Buffer overflow reading float value".to_string());
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&data[*cursor..*cursor + 8]);
            *cursor += 8;
            Ok(Value::Float(f64::from_be_bytes(bytes)))
        }
        4 => {
            if *cursor + 1 > data.len() {
                return Err("Buffer overflow reading bool value".to_string());
            }
            let b = data[*cursor] != 0;
            *cursor += 1;
            Ok(Value::Bool(b))
        }
        other => Err(format!("Unknown value type tag: {}", other)),
    }
}

pub struct PersistenceManager {
    data_dir: PathBuf,
    log_file_path: PathBuf,
    catalog_path: PathBuf,
    next_tx_id: u64,
    log_file: Option<File>,
    // Batched WAL writer
    wal_tx: Option<Sender<BatchRequest>>,
    wal_handle: Option<thread::JoinHandle<()>>,
}

#[derive(Debug)]
enum BatchRequest {
    Op(LogOp),
    Flush(Sender<Result<u64, String>>),
    Shutdown,
}

impl PersistenceManager {
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
        let dir = data_dir.as_ref().to_path_buf();
        PersistenceManager {
            log_file_path: dir.join("operations.log"),
            catalog_path: dir.join("catalog.db"),
            data_dir: dir,
            next_tx_id: 1,
            log_file: None,
            wal_tx: None,
            wal_handle: None,
        }
    }

    pub fn init(&mut self) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir)
            .map_err(|e| format!("Failed to create data directory: {}", e))?;
        
        // Start the batched WAL writer thread
        let log_file_path = self.log_file_path.clone();
        let (tx, rx) = mpsc::channel::<BatchRequest>();
        
        let handle = thread::spawn(move || {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file_path)
                .map_err(|e| format!("Failed to open log file: {}", e)).unwrap();
            
            let mut batch = Vec::new();
            let mut last_flush = std::time::Instant::now();
            const BATCH_MAX_SIZE: usize = 100;
            const FLUSH_INTERVAL_MS: u64 = 10;
            
            loop {
                match rx.recv_timeout(Duration::from_millis(FLUSH_INTERVAL_MS)) {
                    Ok(BatchRequest::Op(op)) => {
                        batch.push(op);
                    }
                    Ok(BatchRequest::Flush(response_tx)) => {
                        // Flush current batch
                        if !batch.is_empty() {
                            let _ = Self::write_batch(&mut file, &mut batch, &mut 0u64);
                        }
                        let _ = response_tx.send(Ok(0));
                    }
                    Ok(BatchRequest::Shutdown) => {
                        // Final flush
                        if !batch.is_empty() {
                            let _ = Self::write_batch(&mut file, &mut batch, &mut 0u64);
                        }
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Periodic flush
                        if !batch.is_empty() && last_flush.elapsed() >= Duration::from_millis(FLUSH_INTERVAL_MS) {
                            let _ = Self::write_batch(&mut file, &mut batch, &mut 0u64);
                            last_flush = std::time::Instant::now();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });
        
        self.wal_tx = Some(tx);
        self.wal_handle = Some(handle);
        Ok(())
    }
    
    fn write_batch(file: &mut File, batch: &mut Vec<LogOp>, tx_id_counter: &mut u64) -> Result<u64, String> {
        if batch.is_empty() {
            return Ok(0);
        }
        
        let batch_op = LogOp::Batch(batch.drain(..).collect());
        let tx_id = *tx_id_counter;
        *tx_id_counter += 1;
        
        let payload = batch_op.serialize();
        let mut entry = Vec::new();
        entry.extend_from_slice(MAGIC_BYTES);
        entry.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        entry.extend_from_slice(&tx_id.to_be_bytes());
        entry.extend(payload);
        
        file.write_all(&entry)
            .map_err(|e| format!("Failed to write batch to log file: {}", e))?;
        file.flush()
            .map_err(|e| format!("Failed to flush log file: {}", e))?;
        
        Ok(tx_id)
    }

    pub fn append_op(&mut self, op: &LogOp) -> Result<u64, String> {
        let tx_id = self.next_tx_id;
        self.next_tx_id += 1;
        
        if let Some(ref tx) = self.wal_tx {
            tx.send(BatchRequest::Op(op.clone()))
                .map_err(|e| format!("Failed to send to WAL thread: {}", e))?;
            // For now, we don't wait for the flush response
            // This gives us async batched writes
            return Ok(tx_id);
        }
        
        // Fallback to direct write if WAL thread not started
        let payload = op.serialize();
        let mut entry = Vec::new();
        entry.extend_from_slice(MAGIC_BYTES);
        entry.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        entry.extend_from_slice(&tx_id.to_be_bytes());
        entry.extend(payload);

        let file = if let Some(ref mut f) = self.log_file {
            f
        } else {
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.log_file_path)
                .map_err(|e| format!("Failed to open log file: {}", e))?;
            self.log_file = Some(f);
            self.log_file.as_mut().unwrap()
        };

        file.write_all(&entry)
            .map_err(|e| format!("Failed to write to log file: {}", e))?;
        file.flush()
            .map_err(|e| format!("Failed to flush log file: {}", e))?;

        Ok(tx_id)
    }
    
    pub fn flush(&mut self) -> Result<(), String> {
        if let Some(ref tx) = self.wal_tx {
            let (response_tx, response_rx) = mpsc::channel();
            tx.send(BatchRequest::Flush(response_tx))
                .map_err(|e| format!("Failed to send flush request: {}", e))?;
            response_rx.recv()
                .map_err(|e| format!("Failed to receive flush response: {}", e))??;
        }
        Ok(())
    }
    
    pub fn shutdown(&mut self) -> Result<(), String> {
        if let Some(tx) = self.wal_tx.take() {
            tx.send(BatchRequest::Shutdown)
                .map_err(|e| format!("Failed to send shutdown: {}", e))?;
        }
        if let Some(handle) = self.wal_handle.take() {
            handle.join().map_err(|e| format!("WAL thread panicked: {:?}", e))?;
        }
        Ok(())
    }
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
    pub fn checkpoint(&mut self, engine: &StorageEngine) -> Result<(), String> {
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

        // Close the cached log file so we can recreate it and release locks
        self.log_file = None;

        // 3. Truncate operations log
        if self.log_file_path.exists() {
            let file = File::create(&self.log_file_path)
                .map_err(|e| format!("Failed to truncate log file: {}", e))?;
            file.set_len(0)
                .map_err(|e| format!("Failed to resize log file: {}", e))?;
        }

        self.next_tx_id = 1;
        Ok(())
    }

    /// Restore the database state from catalog.db, <db>.db files, and replay operations.log.
    pub fn restore(&mut self, engine: &mut StorageEngine) -> Result<(), String> {
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
                LogOp::Batch(ops) => {
                    for op in ops {
                        match op {
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
                            LogOp::Batch(_) => {
                                // Nested batches not supported
                            }
                        }
                    }
                }
            }
            self.next_tx_id = tx_id + 1;
        }

        Ok(())
    }
}
