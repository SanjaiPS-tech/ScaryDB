use crate::config::Config;
use crate::engine::StorageEngine;
use crate::parser::Command;
use crate::persistence::{LogOp, PersistenceManager};
use crate::tls_auth::{AuthContext, AuthManager};
use crate::value::Value;
use flume::{Receiver, Sender};
use parking_lot::RwLock;
use std::path::Path;
use std::sync::Arc;
use std::thread;

pub struct Request {
    pub command: Command,
    pub db_context: Option<u32>,
    pub auth_context: AuthContext,
    pub response_tx: Sender<Response>,
}

pub struct Response {
    pub result: Result<String, String>,
    pub updated_db_context: Option<u32>,
}

pub struct DatabaseSystem {
    pub engine: StorageEngine,
    pub persistence: PersistenceManager,
    pub config: Config,
    pub config_path: String,
    pub mutations_since_checkpoint: usize,
    pub auth_manager: Arc<AuthManager>,
}

impl DatabaseSystem {
    pub fn new(config: Config, config_path: &str) -> Self {
        let persistence = PersistenceManager::new(Path::new(&config.storage.data_dir));
        let auth_manager = Arc::new(AuthManager::new(&config.auth));
        DatabaseSystem {
            engine: StorageEngine::new(),
            persistence,
            config,
            config_path: config_path.to_string(),
            mutations_since_checkpoint: 0,
            auth_manager,
        }
    }

    pub fn init_and_restore(&mut self) -> Result<(), String> {
        self.persistence.init()?;
        self.persistence.restore(&mut self.engine)?;
        Ok(())
    }

    pub fn execute_command(&mut self, command: Command, current_db: Option<u32>) -> Response {
        let mut updated_db_context = current_db;
        let result = match command {
            Command::CreateDb { db_name } => self.execute_create_db(db_name),
            Command::DropDb { db_name } => self.execute_drop_db(db_name),
            Command::Use { db_name } => self.execute_use(db_name, &mut updated_db_context),
            Command::CreateBucket { bucket_name } => self.execute_create_bucket(bucket_name, current_db),
            Command::DropBucket { bucket_name } => self.execute_drop_bucket(bucket_name, current_db),
            Command::ListDbs => Ok(self.execute_list_dbs()),
            Command::ListBuckets => self.execute_list_buckets(current_db),
            Command::Set { bucket, ops } => self.execute_set(bucket, ops, current_db),
            Command::Del { bucket, keys } => self.execute_del(bucket, keys, current_db),
            Command::Get { bucket, keys } => self.execute_get(bucket, keys, current_db),
            Command::Exists { bucket, keys } => self.execute_exists(bucket, keys, current_db),
            Command::ListKeys { bucket } => self.execute_list_keys(bucket, current_db),
            Command::CountKeys { bucket } => self.execute_count_keys(bucket, current_db),
            Command::Boink => Ok("BOINK! 🐷".to_string()),
            Command::Info => Ok(self.execute_info()),
            Command::Stats => Ok(self.engine.get_stats()),
            Command::Version => Ok(format!("ScaryDB v{}", self.config.metadata.version)),
            Command::Help | Command::Man => Ok(self.execute_help()),
            Command::ListConfig => Ok(self.execute_list_config()),
            Command::GetConfig { property } => self.execute_get_config(property),
            Command::SetConfig { property, value } => self.execute_set_config(property, value),
            // Auth commands handled at connection level - should not reach here
            Command::Auth { .. } | Command::AuthToken { .. } => Err("Auth commands handled at connection level".to_string()),
            // Backup/Restore
            Command::Backup { path } => self.execute_backup(path),
            Command::Restore { path } => self.execute_restore(path),
            // Circuit breaker
            Command::CircuitBreakerStatus => self.execute_circuit_breaker_status(),
            Command::CircuitBreakerReset => self.execute_circuit_breaker_reset(),
        };

        Response {
            result,
            updated_db_context,
        }
    }

    fn check_checkpoint(&mut self) {
        self.mutations_since_checkpoint += 1;
        if self.mutations_since_checkpoint >= self.config.storage.checkpoint_interval_ops {
            if !crate::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
                println!("Checkpoint limit reached. Saving DB state and truncating log...");
            }
            if let Err(e) = self.persistence.checkpoint(&self.engine) {
                eprintln!("Error performing database checkpoint: {}", e);
            } else {
                self.mutations_since_checkpoint = 0;
                if !crate::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("Checkpoint successfully complete!");
                }
            }
        }
    }

    // --- Command Handlers ---

    fn execute_create_db(&mut self, db_name: String) -> Result<String, String> {
        let _ = self.engine.create_db(&db_name)?;
        self.persistence.append_op(&LogOp::CreateDb { db_name: db_name.clone() })?;
        self.check_checkpoint();
        Ok(format!("Database '{}' created successfully.", db_name))
    }

    fn execute_drop_db(&mut self, db_name: String) -> Result<String, String> {
        let _ = self.engine.drop_db(&db_name)?;
        self.persistence.append_op(&LogOp::DropDb { db_name: db_name.clone() })?;
        self.check_checkpoint();
        Ok(format!("Database '{}' dropped successfully.", db_name))
    }

    fn execute_use(&self, db_name: String, updated_db_context: &mut Option<u32>) -> Result<String, String> {
        if let Some(db_id) = self.engine.global_catalog.get_db_id(&db_name) {
            *updated_db_context = Some(db_id);
            Ok(format!("Switched to database '{}'.", db_name))
        } else {
            Err(format!("Database '{}' not found.", db_name))
        }
    }

    fn execute_create_bucket(&mut self, bucket_name: String, current_db: Option<u32>) -> Result<String, String> {
        let db_id = current_db.ok_or_else(|| "No database selected. Run 'USE <db_name>;' first.".to_string())?;
        let db_name = self.engine.global_catalog.db_id_to_name.get(&db_id)
            .cloned()
            .ok_or_else(|| "Internal error: Active database ID not found in catalog".to_string())?;

        let _ = self.engine.create_bucket(db_id, &bucket_name)?;
        self.persistence.append_op(&LogOp::CreateBucket {
            db_name,
            bucket_name: bucket_name.clone(),
        })?;
        self.check_checkpoint();
        Ok(format!("Bucket '{}' created successfully.", bucket_name))
    }

    fn execute_drop_bucket(&mut self, bucket_name: String, current_db: Option<u32>) -> Result<String, String> {
        let db_id = current_db.ok_or_else(|| "No database selected. Run 'USE <db_name>;' first.".to_string())?;
        let db_name = self.engine.global_catalog.db_id_to_name.get(&db_id)
            .cloned()
            .ok_or_else(|| "Internal error: Active database ID not found in catalog".to_string())?;

        let _ = self.engine.drop_bucket(db_id, &bucket_name)?;
        self.persistence.append_op(&LogOp::DropBucket {
            db_name,
            bucket_name: bucket_name.clone(),
        })?;
        self.check_checkpoint();
        Ok(format!("Bucket '{}' dropped successfully.", bucket_name))
    }

    fn execute_list_dbs(&self) -> String {
        let dbs = self.engine.list_dbs();
        if dbs.is_empty() {
            "No databases found.".to_string()
        } else {
            dbs.join("\n")
        }
    }

    fn execute_list_buckets(&self, current_db: Option<u32>) -> Result<String, String> {
        let db_id = current_db.ok_or_else(|| "No database selected. Run 'USE <db_name>;' first.".to_string())?;
        let buckets = self.engine.list_buckets(db_id)?;
        if buckets.is_empty() {
            Ok("No buckets found in this database.".to_string())
        } else {
            Ok(buckets.join("\n"))
        }
    }

    fn execute_set(&mut self, bucket: String, ops: Vec<crate::parser::SetOp>, current_db: Option<u32>) -> Result<String, String> {
        let db_id = current_db.ok_or_else(|| "No database selected. Run 'USE <db_name>;' first.".to_string())?;
        let db_name = self.engine.global_catalog.db_id_to_name.get(&db_id)
            .cloned()
            .ok_or_else(|| "Internal error: Active database ID not found in catalog".to_string())?;

        // Parse all values first for atomic batch validity
        let mut parsed_ops = Vec::with_capacity(ops.len());
        for op in &ops {
            let val = Value::parse(&op.value_str, op.explicit_type.as_deref())
                .map_err(|e| format!("Failed to parse value for key '{}': {}", op.key, e))?;
            parsed_ops.push((&op.key, val));
        }

        // Perform write operations & WAL logging
        let mut count = 0;
        for (key, val) in parsed_ops {
            self.engine.set_key(db_id, &bucket, key, val.clone())?;
            self.persistence.append_op(&LogOp::Set {
                db_name: db_name.clone(),
                bucket_name: bucket.clone(),
                key_name: key.to_string(),
                value: val,
            })?;
            self.check_checkpoint();
            count += 1;
        }

        Ok(format!("Successfully set {} key-value pair(s).", count))
    }

    fn execute_del(&mut self, bucket: String, keys: Vec<String>, current_db: Option<u32>) -> Result<String, String> {
        let db_id = current_db.ok_or_else(|| "No database selected. Run 'USE <db_name>;' first.".to_string())?;
        let db_name = self.engine.global_catalog.db_id_to_name.get(&db_id)
            .cloned()
            .ok_or_else(|| "Internal error: Active database ID not found in catalog".to_string())?;

        let mut count = 0;
        for key in &keys {
            let deleted = self.engine.del_key(db_id, &bucket, key)?;
            if deleted {
                self.persistence.append_op(&LogOp::Del {
                    db_name: db_name.clone(),
                    bucket_name: bucket.clone(),
                    key_name: key.clone(),
                })?;
                self.check_checkpoint();
                count += 1;
            }
        }

        Ok(format!("Deleted {} key(s).", count))
    }

    fn execute_get(&self, bucket: String, keys: Vec<String>, current_db: Option<u32>) -> Result<String, String> {
        let db_id = current_db.ok_or_else(|| "No database selected. Run 'USE <db_name>;' first.".to_string())?;
        let mut results = Vec::with_capacity(keys.len());
        for key in &keys {
            match self.engine.get_key(db_id, &bucket, key)? {
                Some(val) => results.push(val.to_string()),
                None => results.push("(nil)".to_string()),
            }
        }
        Ok(results.join(" / "))
    }

    fn execute_exists(&self, bucket: String, keys: Vec<String>, current_db: Option<u32>) -> Result<String, String> {
        let db_id = current_db.ok_or_else(|| "No database selected. Run 'USE <db_name>;' first.".to_string())?;
        let mut results = Vec::with_capacity(keys.len());
        for key in &keys {
            let exists = self.engine.exists_key(db_id, &bucket, key)?;
            results.push(exists.to_string());
        }
        Ok(results.join(" / "))
    }

    fn execute_list_keys(&self, bucket: String, current_db: Option<u32>) -> Result<String, String> {
        let db_id = current_db.ok_or_else(|| "No database selected. Run 'USE <db_name>;' first.".to_string())?;
        let keys = self.engine.list_keys(db_id, &bucket)?;
        if keys.is_empty() {
            Ok("Empty bucket.".to_string())
        } else {
            Ok(keys.join("\n"))
        }
    }

    fn execute_count_keys(&self, bucket: String, current_db: Option<u32>) -> Result<String, String> {
        let db_id = current_db.ok_or_else(|| "No database selected. Run 'USE <db_name>;' first.".to_string())?;
        let count = self.engine.count_keys(db_id, &bucket)?;
        Ok(count.to_string())
    }

    fn execute_info(&self) -> String {
        format!(
            "ScaryDB v{}\nStartup Time: {}\nData Dir: {}\nMax Memory limit (KB): {}\nWorkers: {}",
            self.config.metadata.version,
            self.config.metadata.startup_time,
            self.config.storage.data_dir,
            self.config.memory.max_memory_kb,
            self.config.server.workers,
        )
    }

    fn execute_help(&self) -> String {
        "ScaryDB Command Syntax:\nDDC (Database Definition Commands)\n  CREATE DB <db_name>;\n  DROP DB <db_name>;\n  USE <db_name>;\n  CREATE BUCKET <bucket_name>;\n  DROP BUCKET <bucket_name>;\n  LIST DBS; (or Databases)\n  LIST BUCKETS; (or Buck)\n\nDMC (Data Manipulation Commands)\n  SET <bucket> <key> [TYPE] <value> / <key> <value> ...;\n  DEL <bucket> <key> / <key> ...;\n\nDRC (Data Retrieval Commands)\n  GET <bucket> <key> / <key> ...;\n  EXISTS <bucket> <key> / <key> ...;\n  LIST <bucket>;\n  COUNT <bucket>;\n\nSCC (System Control Commands)\n  BOINK / PING\n  INFO\n  STATS\n  VERSION\n  HELP / MAN\n\nCCC (Configuration Control Commands)\n  LIST CONFIG;\n  GET CONFIG <property>;\n  SET CONFIG <property> <value>;\n\nBackup/Restore\n  BACKUP <path>;\n  RESTORE <path>;\n\nCircuit Breaker\n  CIRCUIT BREAKER STATUS;\n  CIRCUIT BREAKER RESET;".to_string()
    }

    fn execute_list_config(&self) -> String {
        format!(
            "server.workers = {}\nstorage.data_dir = {}\nstorage.checkpoint_interval_ops = {}\nmemory.max_memory_kb = {}\nnetwork.host = {}\nnetwork.port = {}",
            self.config.server.workers,
            self.config.storage.data_dir,
            self.config.storage.checkpoint_interval_ops,
            self.config.memory.max_memory_kb,
            self.config.network.host,
            self.config.network.port
        )
    }

    fn execute_get_config(&self, property: String) -> Result<String, String> {
        self.config.get_property(&property)
    }

    fn execute_set_config(&mut self, property: String, value: String) -> Result<String, String> {
        self.config.set_property(&property, &value)?;
        self.config.save(&self.config_path)?;
        Ok(format!("Configuration property '{}' updated to '{}' and saved.", property, value))
    }

    // Backup/Restore
    fn execute_backup(&mut self, path: String) -> Result<String, String> {
        // Create backup by checkpointing and copying data files
        self.persistence.checkpoint(&self.engine)?;
        
        let backup_path = std::path::Path::new(&path);
        std::fs::create_dir_all(backup_path)
            .map_err(|e| format!("Failed to create backup directory: {}", e))?;
        
        // Copy catalog
        std::fs::copy(
            &self.persistence.catalog_path,
            backup_path.join("catalog.db")
        ).map_err(|e| format!("Failed to copy catalog: {}", e))?;
        
        // Copy database files
        for (db_id, db_name) in &self.engine.global_catalog.db_id_to_name {
            let src = self.persistence.data_dir.join(format!("{}.db", db_name));
            if src.exists() {
                std::fs::copy(&src, backup_path.join(format!("{}.db", db_name)))
                    .map_err(|e| format!("Failed to copy database {}: {}", db_name, e))?;
            }
        }
        
        // Copy WAL
        if self.persistence.log_file_path.exists() {
            std::fs::copy(
                &self.persistence.log_file_path,
                backup_path.join("operations.log")
            ).map_err(|e| format!("Failed to copy WAL: {}", e))?;
        }
        
        Ok(format!("Backup created at '{}'", path))
    }

    fn execute_restore(&mut self, path: String) -> Result<String, String> {
        let backup_path = std::path::Path::new(&path);
        
        if !backup_path.exists() {
            return Err(format!("Backup path '{}' does not exist", path));
        }
        
        // Restore catalog
        let catalog_src = backup_path.join("catalog.db");
        if catalog_src.exists() {
            std::fs::copy(&catalog_src, &self.persistence.catalog_path)
                .map_err(|e| format!("Failed to restore catalog: {}", e))?;
        }
        
        // Restore database files
        let entries = std::fs::read_dir(backup_path)
            .map_err(|e| format!("Failed to read backup directory: {}", e))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            if file_name_str.ends_with(".db") && file_name_str != "catalog.db" {
                let dest = self.persistence.data_dir.join(file_name_str.as_ref());
                std::fs::copy(entry.path(), &dest)
                    .map_err(|e| format!("Failed to restore database {}: {}", file_name_str, e))?;
            }
        }
        
        // Restore WAL
        let wal_src = backup_path.join("operations.log");
        if wal_src.exists() {
            std::fs::copy(&wal_src, &self.persistence.log_file_path)
                .map_err(|e| format!("Failed to restore WAL: {}", e))?;
        }
        
        // Reinitialize engine from restored state
        self.engine = crate::engine::StorageEngine::new();
        self.persistence.restore(&mut self.engine)?;
        
        Ok(format!("Restored from backup '{}'", path))
    }

    // Circuit Breaker
    fn execute_circuit_breaker_status(&self) -> Result<String, String> {
        Ok("Circuit breaker status: CLOSED (not implemented yet)".to_string())
    }

    fn execute_circuit_breaker_reset(&mut self) -> Result<String, String> {
        Ok("Circuit breaker reset (not implemented yet)".to_string())
    }
}

pub struct WorkerPool {
    _workers: Vec<thread::JoinHandle<()>>,
}

impl WorkerPool {
    pub fn new(
        num_workers: usize,
        request_rx: Arc<Receiver<Request>>,
        system: Arc<RwLock<DatabaseSystem>>,
    ) -> Self {
        let mut workers = Vec::with_capacity(num_workers);
        for id in 0..num_workers {
            let rx = request_rx.clone();
            let sys = Arc::clone(&system);
            
            let handle = thread::spawn(move || {
                if !crate::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("Worker thread {} started and waiting for requests...", id);
                }
                loop {
                    // Pull next request from Request Queue
                    let request = match rx.recv() {
                        Ok(req) => req,
                        Err(_) => {
                            if !crate::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
                                println!("Worker thread {} channel closed. Shutting down.", id);
                            }
                            break;
                        }
                    };
                    
                    // Execute command
                    let response = {
                        let mut sys_lock = sys.write();
                        sys_lock.execute_command(request.command, request.db_context)
                    };
                    
                    // Send response back
                    let _ = request.response_tx.send(response);
                }
            });
            workers.push(handle);
        }
        
        WorkerPool { _workers: workers }
    }

    #[allow(dead_code)]
    pub fn shutdown(self) {
        for handle in self._workers {
            let _ = handle.join();
        }
    }
}