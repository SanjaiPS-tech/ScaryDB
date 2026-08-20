use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub server: ServerSettings,
    pub storage: StorageSettings,
    pub memory: MemorySettings,
    pub network: NetworkSettings,
    pub tls: TlsSettings,
    pub auth: AuthSettings,
    pub metadata: RuntimeMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerSettings {
    pub workers: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageSettings {
    pub data_dir: String,
    pub checkpoint_interval_ops: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemorySettings {
    pub max_memory_kb: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TlsSettings {
    pub enabled: bool,
    pub cert_file: String,
    pub key_file: String,
    pub ca_file: Option<String>,
    pub require_client_cert: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthSettings {
    pub enabled: bool,
    pub jwt_secret: String,
    pub api_keys: Vec<String>,
    pub token_expiry_hours: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuntimeMetadata {
    pub version: String,
    pub startup_time: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerSettings { workers: 1 },
            storage: StorageSettings {
                data_dir: "./data".to_string(),
                checkpoint_interval_ops: 100,
            },
            memory: MemorySettings { max_memory_kb: 0 }, // 0 = unlimited
            network: NetworkSettings {
                host: "127.0.0.1".to_string(),
                port: std::env::var("SCARYDB_TEST_PORT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(6379), // default port for custom DB (similar to Redis/Memcached)
            },
            tls: TlsSettings {
                enabled: false,
                cert_file: "./certs/server.crt".to_string(),
                key_file: "./certs/server.key".to_string(),
                ca_file: None,
                require_client_cert: false,
            },
            auth: AuthSettings {
                enabled: false,
                jwt_secret: "change-me-in-production".to_string(),
                api_keys: vec![],
                token_expiry_hours: 24,
            },
            metadata: RuntimeMetadata {
                version: env!("CARGO_PKG_VERSION").to_string(),
                startup_time: Utc::now().to_rfc3339(),
            },
        }
    }
}

impl Config {
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        // Allow test config path override via environment variable
        let config_path = std::env::var("SCARYDB_CONFIG_PATH")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| path.as_ref().to_path_buf());
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .map_err(|e| format!("Failed to read config file: {}", e))?;
            let mut config: Config = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse config file: {}", e))?;
            // Allow test port override via environment variable
            if let Ok(port_str) = std::env::var("SCARYDB_TEST_PORT") {
                if let Ok(port) = port_str.parse::<u16>() {
                    config.network.port = port;
                }
            }
            // Allow test data dir override via environment variable
            if let Ok(data_dir) = std::env::var("SCARYDB_TEST_DATA_DIR") {
                config.storage.data_dir = data_dir;
            }
            // Always set current start time on load/run
            config.metadata.startup_time = Utc::now().to_rfc3339();
            Ok(config)
        } else {
            let config = Config::default();
            config.save(&config_path)?;
            Ok(config)
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config parent directories: {}", e))?;
        }
        fs::write(path, content)
            .map_err(|e| format!("Failed to write config file: {}", e))?;
        Ok(())
    }

    pub fn get_property(&self, prop: &str) -> Result<String, String> {
        let parts: Vec<&str> = prop.split('.').collect();
        if parts.is_empty() {
            return Err("Empty configuration property".to_string());
        }

        match parts[0] {
            "server" => {
                if parts.len() < 2 {
                    return Err("Incomplete server property path".to_string());
                }
                match parts[1] {
                    "workers" => Ok(self.server.workers.to_string()),
                    _ => Err(format!("Unknown server property: {}", parts[1])),
                }
            }
            "storage" => {
                if parts.len() < 2 {
                    return Err("Incomplete storage property path".to_string());
                }
                match parts[1] {
                    "data_dir" => Ok(self.storage.data_dir.clone()),
                    "checkpoint_interval_ops" => Ok(self.storage.checkpoint_interval_ops.to_string()),
                    _ => Err(format!("Unknown storage property: {}", parts[1])),
                }
            }
            "memory" => {
                if parts.len() < 2 {
                    return Err("Incomplete memory property path".to_string());
                }
                match parts[1] {
                    "max_memory_kb" => Ok(self.memory.max_memory_kb.to_string()),
                    _ => Err(format!("Unknown memory property: {}", parts[1])),
                }
            }
            "network" => {
                if parts.len() < 2 {
                    return Err("Incomplete network property path".to_string());
                }
                match parts[1] {
                    "host" => Ok(self.network.host.clone()),
                    "port" => Ok(self.network.port.to_string()),
                    _ => Err(format!("Unknown network property: {}", parts[1])),
                }
            }
            "metadata" => {
                if parts.len() < 2 {
                    return Err("Incomplete metadata property path".to_string());
                }
                match parts[1] {
                    "version" => Ok(self.metadata.version.clone()),
                    "startup_time" => Ok(self.metadata.startup_time.clone()),
                    _ => Err(format!("Unknown metadata property: {}", parts[1])),
                }
            }
            _ => Err(format!("Unknown configuration group: {}", parts[0])),
        }
    }

    pub fn set_property(&mut self, prop: &str, value: &str) -> Result<(), String> {
        let parts: Vec<&str> = prop.split('.').collect();
        if parts.is_empty() {
            return Err("Empty configuration property".to_string());
        }

        match parts[0] {
            "server" => {
                if parts.len() < 2 {
                    return Err("Incomplete server property path".to_string());
                }
                match parts[1] {
                    "workers" => {
                        let workers = value.parse::<usize>().map_err(|_| "Value must be a valid integer")?;
                        self.server.workers = workers;
                        Ok(())
                    }
                    _ => Err(format!("Unknown server property: {}", parts[1])),
                }
            }
            "storage" => {
                if parts.len() < 2 {
                    return Err("Incomplete storage property path".to_string());
                }
                match parts[1] {
                    "data_dir" => {
                        self.storage.data_dir = value.to_string();
                        Ok(())
                    }
                    "checkpoint_interval_ops" => {
                        let interval = value.parse::<usize>().map_err(|_| "Value must be a valid integer")?;
                        self.storage.checkpoint_interval_ops = interval;
                        Ok(())
                    }
                    _ => Err(format!("Unknown storage property: {}", parts[1])),
                }
            }
            "memory" => {
                if parts.len() < 2 {
                    return Err("Incomplete memory property path".to_string());
                }
                match parts[1] {
                    "max_memory_kb" => {
                        let max_mem = value.parse::<usize>().map_err(|_| "Value must be a valid integer")?;
                        self.memory.max_memory_kb = max_mem;
                        Ok(())
                    }
                    _ => Err(format!("Unknown memory property: {}", parts[1])),
                }
            }
            "network" => {
                if parts.len() < 2 {
                    return Err("Incomplete network property path".to_string());
                }
                match parts[1] {
                    "host" => {
                        self.network.host = value.to_string();
                        Ok(())
                    }
                    "port" => {
                        let port = value.parse::<u16>().map_err(|_| "Value must be a valid port number")?;
                        self.network.port = port;
                        Ok(())
                    }
                    _ => Err(format!("Unknown network property: {}", parts[1])),
                }
            }
            "metadata" => Err("Runtime metadata is read-only".to_string()),
            _ => Err(format!("Unknown configuration group: {}", parts[0])),
        }
    }
}
