use crate::config::Config;
use crate::engine::StorageEngine;
use crate::parser::Command;
use crate::persistence::{LogOp, PersistenceManager};
use crate::value::Value;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Request {
    pub command: Command,
    pub db_context: Option<u32>,
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
}

impl DatabaseSystem {
    pub fn new(config: Config, config_path: &str) -> Self {
        let persistence = PersistenceManager::new(&config.storage.data_dir);
        DatabaseSystem {
            engine: StorageEngine::new(),
            persistence,
            config,
            config_path: config_path.to_string(),
            mutations_since_checkpoint: 0,
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
        };

        Response {
            result,
            updated_db_context,
        }
    }

    fn check_checkpoint(&mut self) {
        self.mutations_since_checkpoint += 1;
        if self.mutations_since_checkpoint >= self.config.storage.checkpoint_interval_ops {
            println!("Checkpoint limit reached. Saving DB state and truncating log...");
            if let Err(e) = self.persistence.checkpoint(&self.engine) {
                eprintln!("Error performing database checkpoint: {}", e);
            } else {
                self.mutations_since_checkpoint = 0;
                println!("Checkpoint successfully complete!");
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

        // 1. Parse all values first to ensure atomic batch validity
        let mut parsed_ops = Vec::new();
        for op in &ops {
            let val = Value::parse(&op.value_str, op.explicit_type.as_deref())
                .map_err(|e| format!("Failed to parse value for key '{}': {}", op.key, e))?;
            parsed_ops.push((&op.key, val));
        }

        // 2. Perform write operations & WAL logging
        let mut count = 0;
        for (key, val) in parsed_ops {
            self.engine.set_key(db_id, &bucket, key, val.clone())?;
            self.persistence.append_op(&LogOp::Set {
                db_name: db_name.clone(),
                bucket_name: bucket.clone(),
                key_name: key.clone(),
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
        let mut results = Vec::new();
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
        let mut results = Vec::new();
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
        "ScaryDB Command Syntax:
DDC (Database Definition Commands)
  CREATE DB <db_name>;
  DROP DB <db_name>;
  USE <db_name>;
  CREATE BUCKET <bucket_name>;
  DROP BUCKET <bucket_name>;
  LIST DBS; (or Databases)
  LIST BUCKETS; (or Buck)

DMC (Data Manipulation Commands)
  SET <bucket> <key> [TYPE] <value> / <key> <value> ...;
  DEL <bucket> <key> / <key> ...;

DRC (Data Retrieval Commands)
  GET <bucket> <key> / <key> ...;
  EXISTS <bucket> <key> / <key> ...;
  LIST <bucket>;
  COUNT <bucket>;

SCC (System Control Commands)
  BOINK / PING
  INFO
  STATS
  VERSION
  HELP / MAN

CCC (Configuration Control Commands)
  LIST CONFIG;
  GET CONFIG <property>;
  SET CONFIG <property> <value>;"
            .to_string()
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
}

pub struct WorkerPool {
    _workers: Vec<thread::JoinHandle<()>>,
}

impl WorkerPool {
    pub fn new(
        num_workers: usize,
        request_rx: Arc<Mutex<Receiver<Request>>>,
        system: Arc<Mutex<DatabaseSystem>>,
    ) -> Self {
        let mut workers = Vec::new();
        for id in 0..num_workers {
            let rx = Arc::clone(&request_rx);
            let sys = Arc::clone(&system);
            
            let handle = thread::spawn(move || {
                println!("Worker thread {} started and waiting for requests...", id);
                loop {
                    // 1. Pull next request from Request Queue
                    let request = {
                        let rx_lock = rx.lock().unwrap();
                        match rx_lock.recv() {
                            Ok(req) => req,
                            Err(_) => {
                                // Channel closed, shutdown worker
                                println!("Worker thread {} channel closed. Shutting down.", id);
                                break;
                            }
                        }
                    };

                    // 2. Execute command
                    let response = {
                        let mut sys_lock = sys.lock().unwrap();
                        sys_lock.execute_command(request.command, request.db_context)
                    };

                    // 3. Send response back
                    let _ = request.response_tx.send(response);
                }
            });
            workers.push(handle);
        }

        WorkerPool { _workers: workers }
    }

    #[allow(dead_code)]
    pub fn shutdown(self) {
        // Since the pool drops, the workers will exit when the channel is dropped.
        for handle in self._workers {
            let _ = handle.join();
        }
    }
}
