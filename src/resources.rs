// Resource Management Module
// Provides resource limits, monitoring, quotas, and graceful degradation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Global resource limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory usage in KB (0 = unlimited)
    pub max_memory_kb: usize,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Maximum number of databases
    pub max_databases: usize,
    /// Maximum number of buckets per database
    pub max_buckets_per_db: usize,
    /// Maximum number of keys per bucket
    pub max_keys_per_bucket: usize,
    /// Maximum key size in bytes
    pub max_key_size_bytes: usize,
    /// Maximum value size in bytes
    pub max_value_size_bytes: usize,
    /// Maximum total disk usage in MB (0 = unlimited)
    pub max_disk_mb: usize,
    /// Connection idle timeout in seconds
    pub connection_idle_timeout_secs: u64,
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    /// Enable resource monitoring
    pub enable_monitoring: bool,
    /// Monitoring interval in seconds
    pub monitoring_interval_secs: u64,
    /// Memory pressure threshold (0.0-1.0, triggers degradation)
    pub memory_pressure_threshold: f64,
    /// Disk pressure threshold (0.0-1.0, triggers degradation)
    pub disk_pressure_threshold: f64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        ResourceLimits {
            max_memory_kb: 0, // unlimited
            max_connections: 10000,
            max_databases: 1000,
            max_buckets_per_db: 10000,
            max_keys_per_bucket: 1000000,
            max_key_size_bytes: 65536,      // 64KB
            max_value_size_bytes: 1048576,  // 1MB
            max_disk_mb: 0,                 // unlimited
            connection_idle_timeout_secs: 300, // 5 minutes
            request_timeout_ms: 5000,       // 5 seconds
            enable_monitoring: true,
            monitoring_interval_secs: 10,
            memory_pressure_threshold: 0.85,
            disk_pressure_threshold: 0.90,
        }
    }
}

/// Resource usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub current_memory_kb: usize,
    pub peak_memory_kb: usize,
    pub current_connections: usize,
    pub peak_connections: usize,
    pub total_databases: usize,
    pub total_buckets: usize,
    pub total_keys: usize,
    pub disk_usage_mb: usize,
    pub uptime_secs: u64,
    pub requests_total: u64,
    pub requests_failed: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub memory_pressure: f64,
    pub disk_pressure: f64,
}

/// Per-database resource quota
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseQuota {
    pub max_buckets: usize,
    pub max_keys: usize,
    pub max_memory_kb: usize,
    pub max_disk_mb: usize,
}

impl Default for DatabaseQuota {
    fn default() -> Self {
        DatabaseQuota {
            max_buckets: 1000,
            max_keys: 100000,
            max_memory_kb: 0, // 0 = use global limit
            max_disk_mb: 0,
        }
    }
}

/// Resource manager for monitoring and enforcing limits
pub struct ResourceManager {
    limits: ResourceLimits,
    usage: Arc<RwLock<ResourceUsage>>,
    db_quotas: Arc<RwLock<HashMap<String, DatabaseQuota>>>,
    start_time: Instant,
    connection_count: AtomicUsize,
    requests_total: AtomicU64,
    requests_failed: AtomicU64,
    bytes_read: AtomicU64,
    bytes_written: AtomicU64,
    peak_memory_kb: AtomicUsize,
    peak_connections: AtomicUsize,
    monitor_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl ResourceManager {
    pub fn new(limits: ResourceLimits) -> Self {
        let now = Instant::now();
        let usage = ResourceUsage {
            current_memory_kb: 0,
            peak_memory_kb: 0,
            current_connections: 0,
            peak_connections: 0,
            total_databases: 0,
            total_buckets: 0,
            total_keys: 0,
            disk_usage_mb: 0,
            uptime_secs: 0,
            requests_total: 0,
            requests_failed: 0,
            bytes_read: 0,
            bytes_written: 0,
            memory_pressure: 0.0,
            disk_pressure: 0.0,
        };

        ResourceManager {
            limits,
            usage: Arc::new(RwLock::new(usage)),
            db_quotas: Arc::new(RwLock::new(HashMap::new())),
            start_time: now,
            connection_count: AtomicUsize::new(0),
            requests_total: AtomicU64::new(0),
            requests_failed: AtomicU64::new(0),
            bytes_read: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            peak_memory_kb: AtomicUsize::new(0),
            peak_connections: AtomicUsize::new(0),
            monitor_handle: Mutex::new(None),
        }
    }

    /// Start background resource monitoring
    pub fn start_monitoring(&self) {
        if !self.limits.enable_monitoring {
            return;
        }

        let usage = Arc::clone(&self.usage);
        let limits = self.limits.clone();
        let start_time = self.start_time;
        let connection_count = Arc::new(AtomicUsize::new(0));
        let requests_total = Arc::new(AtomicU64::new(0));
        let requests_failed = Arc::new(AtomicU64::new(0));
        let bytes_read = Arc::new(AtomicU64::new(0));
        let bytes_written = Arc::new(AtomicU64::new(0));
        let peak_memory_kb = Arc::new(AtomicUsize::new(0));
        let peak_connections = Arc::new(AtomicUsize::new(0));
        let db_quotas = Arc::clone(&self.db_quotas);

        let handle = std::thread::spawn(move || {
            let interval = Duration::from_secs(limits.monitoring_interval_secs);
            loop {
                std::thread::sleep(interval);
                
                let mut usage_guard = usage.write().unwrap();
                usage_guard.uptime_secs = start_time.elapsed().as_secs();
                usage_guard.current_connections = connection_count.load(Ordering::Relaxed);
                usage_guard.requests_total = requests_total.load(Ordering::Relaxed);
                usage_guard.requests_failed = requests_failed.load(Ordering::Relaxed);
                usage_guard.bytes_read = bytes_read.load(Ordering::Relaxed);
                usage_guard.bytes_written = bytes_written.load(Ordering::Relaxed);
                
                // Update peak values
                let current_mem = usage_guard.current_memory_kb;
                let current_peak = peak_memory_kb.load(Ordering::Relaxed);
                if current_mem > current_peak {
                    peak_memory_kb.store(current_mem, Ordering::Relaxed);
                    usage_guard.peak_memory_kb = current_mem;
                }
                
                let current_conn = usage_guard.current_connections;
                let conn_peak = peak_connections.load(Ordering::Relaxed);
                if current_conn > conn_peak {
                    peak_connections.store(current_conn, Ordering::Relaxed);
                    usage_guard.peak_connections = current_conn;
                }

                // Calculate pressure levels
                if limits.max_memory_kb > 0 {
                    usage_guard.memory_pressure = current_mem as f64 / limits.max_memory_kb as f64;
                }
                
                if limits.max_disk_mb > 0 {
                    usage_guard.disk_pressure = usage_guard.disk_usage_mb as f64 / limits.max_disk_mb as f64;
                }

                // Check pressure thresholds and warn
                if usage_guard.memory_pressure > limits.memory_pressure_threshold {
                    warn!(
                        memory_pressure = usage_guard.memory_pressure,
                        threshold = limits.memory_pressure_threshold,
                        "High memory pressure detected"
                    );
                }
                
                if usage_guard.disk_pressure > limits.disk_pressure_threshold {
                    warn!(
                        disk_pressure = usage_guard.disk_pressure,
                        threshold = limits.disk_pressure_threshold,
                        "High disk pressure detected"
                    );
                }

                // Log current usage periodically
                info!(
                    memory_kb = usage_guard.current_memory_kb,
                    connections = usage_guard.current_connections,
                    databases = usage_guard.total_databases,
                    buckets = usage_guard.total_buckets,
                    keys = usage_guard.total_keys,
                    memory_pressure = usage_guard.memory_pressure,
                    disk_pressure = usage_guard.disk_pressure,
                    "Resource usage snapshot"
                );
            }
        });

        *self.monitor_handle.lock().unwrap() = Some(handle);
    }

    /// Stop background monitoring
    pub fn stop_monitoring(&self) {
        if let Some(handle) = self.monitor_handle.lock().unwrap().take() {
            handle.join().ok();
        }
    }

    /// Check if new connection can be accepted
    pub fn try_acquire_connection(&self) -> Result<ConnectionGuard, ResourceError> {
        let current = self.connection_count.load(Ordering::Relaxed);
        if current >= self.limits.max_connections {
            self.requests_failed.fetch_add(1, Ordering::Relaxed);
            return Err(ResourceError::MaxConnectionsExceeded);
        }
        
        self.connection_count.fetch_add(1, Ordering::Relaxed);
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        
        Ok(ConnectionGuard {
            manager: self,
        })
    }

    /// Register a new database
    pub fn register_database(&self, name: &str) -> Result<(), ResourceError> {
        let mut usage = self.usage.write().unwrap();
        if usage.total_databases >= self.limits.max_databases {
            return Err(ResourceError::MaxDatabasesExceeded);
        }
        usage.total_databases += 1;
        Ok(())
    }

    /// Unregister a database
    pub fn unregister_database(&self, name: &str) {
        let mut usage = self.usage.write().unwrap();
        if usage.total_databases > 0 {
            usage.total_databases -= 1;
        }
        // Clean up quota
        self.db_quotas.write().unwrap().remove(name);
    }

    /// Set quota for a database
    pub fn set_database_quota(&self, db_name: &str, quota: DatabaseQuota) {
        self.db_quotas
            .write()
            .unwrap()
            .insert(db_name.to_string(), quota);
    }

    /// Get quota for a database
    pub fn get_database_quota(&self, db_name: &str) -> DatabaseQuota {
        self.db_quotas
            .read()
            .unwrap()
            .get(db_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if bucket creation is allowed
    pub fn try_create_bucket(&self, db_name: &str) -> Result<(), ResourceError> {
        let quota = self.get_database_quota(db_name);
        let mut usage = self.usage.write().unwrap();
        
        if usage.total_buckets >= self.limits.max_buckets_per_db {
            return Err(ResourceError::MaxBucketsExceeded);
        }
        
        if quota.max_buckets > 0 && usage.total_buckets >= quota.max_buckets {
            return Err(ResourceError::DatabaseQuotaExceeded("buckets".to_string()));
        }
        
        usage.total_buckets += 1;
        Ok(())
    }

    /// Check if key creation is allowed
    pub fn try_create_key(
        &self,
        db_name: &str,
        key_size: usize,
        value_size: usize,
    ) -> Result<(), ResourceError> {
        let quota = self.get_database_quota(db_name);
        let mut usage = self.usage.write().unwrap();
        
        if key_size > self.limits.max_key_size_bytes {
            return Err(ResourceError::KeyTooLarge);
        }
        
        if value_size > self.limits.max_value_size_bytes {
            return Err(ResourceError::ValueTooLarge);
        }
        
        if quota.max_keys > 0 && usage.total_keys >= quota.max_keys {
            return Err(ResourceError::DatabaseQuotaExceeded("keys".to_string()));
        }
        
        if self.limits.max_keys_per_bucket > 0
            && usage.total_keys >= self.limits.max_keys_per_bucket
        {
            return Err(ResourceError::MaxKeysExceeded);
        }
        
        // Check memory pressure
        if self.is_under_memory_pressure() {
            return Err(ResourceError::MemoryPressure);
        }
        
        usage.total_keys += 1;
        Ok(())
    }

    /// Record successful request
    pub fn record_request_success(&self, duration: Duration, bytes_read: u64, bytes_written: u64) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.bytes_read.fetch_add(bytes_read, Ordering::Relaxed);
        self.bytes_written
            .fetch_add(bytes_written, Ordering::Relaxed);
        
        // Update usage
        let mut usage = self.usage.write().unwrap();
        usage.requests_total = self.requests_total.load(Ordering::Relaxed);
        usage.bytes_read = self.bytes_read.load(Ordering::Relaxed);
        usage.bytes_written = self.bytes_written.load(Ordering::Relaxed);
    }

    /// Record failed request
    pub fn record_request_failed(&self) {
        self.requests_failed.fetch_add(1, Ordering::Relaxed);
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Update memory usage
    pub fn update_memory_usage(&self, memory_kb: usize) {
        let mut usage = self.usage.write().unwrap();
        usage.current_memory_kb = memory_kb;
        
        let current_peak = self.peak_memory_kb.load(Ordering::Relaxed);
        if memory_kb > current_peak {
            self.peak_memory_kb.store(memory_kb, Ordering::Relaxed);
            usage.peak_memory_kb = memory_kb;
        }
        
        if self.limits.max_memory_kb > 0 {
            usage.memory_pressure = memory_kb as f64 / self.limits.max_memory_kb as f64;
        }
    }

    /// Update disk usage
    pub fn update_disk_usage(&self, disk_mb: usize) {
        let mut usage = self.usage.write().unwrap();
        usage.disk_usage_mb = disk_mb;
        
        if self.limits.max_disk_mb > 0 {
            usage.disk_pressure = disk_mb as f64 / self.limits.max_disk_mb as f64;
        }
    }

    /// Check if under memory pressure
    pub fn is_under_memory_pressure(&self) -> bool {
        let usage = self.usage.read().unwrap();
        usage.memory_pressure > self.limits.memory_pressure_threshold
    }

    /// Check if under disk pressure
    pub fn is_under_disk_pressure(&self) -> bool {
        let usage = self.usage.read().unwrap();
        usage.disk_pressure > self.limits.disk_pressure_threshold
    }

    /// Get current resource usage
    pub fn get_usage(&self) -> ResourceUsage {
        let mut usage = self.usage.read().unwrap().clone();
        usage.uptime_secs = self.start_time.elapsed().as_secs();
        usage
    }

    /// Get resource limits
    pub fn get_limits(&self) -> ResourceLimits {
        self.limits.clone()
    }

    /// Check if resource limits are exceeded
    pub fn check_limits(&self) -> Vec<ResourceError> {
        let mut errors = Vec::new();
        let usage = self.usage.read().unwrap();
        
        if self.limits.max_memory_kb > 0 && usage.current_memory_kb > self.limits.max_memory_kb {
            errors.push(ResourceError::MemoryLimitExceeded);
        }
        
        if self.limits.max_connections > 0
            && usage.current_connections > self.limits.max_connections
        {
            errors.push(ResourceError::MaxConnectionsExceeded);
        }
        
        if self.limits.max_databases > 0 && usage.total_databases > self.limits.max_databases {
            errors.push(ResourceError::MaxDatabasesExceeded);
        }
        
        if self.limits.max_disk_mb > 0 && usage.disk_usage_mb > self.limits.max_disk_mb {
            errors.push(ResourceError::DiskLimitExceeded);
        }
        
        errors
    }

    /// Trigger garbage collection / cleanup
    pub fn trigger_cleanup(&self) -> CleanupResult {
        info!("Triggering resource cleanup");
        // In a real implementation, this would:
        // - Compact WAL logs
        // - Evict least recently used items if using cache
        // - Force checkpoint
        // - Release unused memory
        
        CleanupResult {
            memory_freed_kb: 0,
            disk_freed_mb: 0,
            items_evicted: 0,
        }
    }
}

/// RAII guard for connection management
pub struct ConnectionGuard<'a> {
    manager: &'a ResourceManager,
}

impl<'a> Drop for ConnectionGuard<'a> {
    fn drop(&mut self) {
        self.manager
            .connection_count
            .fetch_sub(1, Ordering::Relaxed);
    }
}

/// Result of cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    pub memory_freed_kb: usize,
    pub disk_freed_mb: usize,
    pub items_evicted: usize,
}

/// Resource management errors
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum ResourceError {
    #[error("Maximum connections exceeded")]
    MaxConnectionsExceeded,
    #[error("Maximum databases exceeded")]
    MaxDatabasesExceeded,
    #[error("Maximum buckets per database exceeded")]
    MaxBucketsExceeded,
    #[error("Maximum keys per bucket exceeded")]
    MaxKeysExceeded,
    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,
    #[error("Disk limit exceeded")]
    DiskLimitExceeded,
    #[error("Key size exceeds limit")]
    KeyTooLarge,
    #[error("Value size exceeds limit")]
    ValueTooLarge,
    #[error("Under memory pressure, rejecting request")]
    MemoryPressure,
    #[error("Under disk pressure, rejecting request")]
    DiskPressure,
    #[error("Database quota exceeded: {0}")]
    DatabaseQuotaExceeded(String),
    #[error("Connection timeout")]
    ConnectionTimeout,
    #[error("Request timeout")]
    RequestTimeout,
}

/// CLI commands for resource management
pub mod commands {
    use super::*;
    use crate::config::Config;

    /// Show resource usage
    pub fn show_resource_usage(manager: &ResourceManager) -> String {
        let usage = manager.get_usage();
        let limits = manager.get_limits();
        
        format!(
            r#"Resource Usage:
  Memory: {} KB / {} KB (peak: {} KB) - Pressure: {:.1}%
  Connections: {} / {} (peak: {})
  Databases: {} / {}
  Buckets: {} / {} per DB
  Keys: {} / {} per bucket
  Disk: {} MB / {} MB - Pressure: {:.1}%
  Uptime: {} seconds
  Requests: {} total, {} failed
  Bytes Read: {} | Bytes Written: {}"#,
            usage.current_memory_kb,
            if limits.max_memory_kb > 0 {
                limits.max_memory_kb.to_string()
            } else {
                "unlimited".to_string()
            },
            usage.peak_memory_kb,
            usage.memory_pressure * 100.0,
            usage.current_connections,
            limits.max_connections,
            usage.peak_connections,
            usage.total_databases,
            limits.max_databases,
            usage.total_buckets,
            limits.max_buckets_per_db,
            usage.total_keys,
            limits.max_keys_per_bucket,
            usage.disk_usage_mb,
            if limits.max_disk_mb > 0 {
                limits.max_disk_mb.to_string()
            } else {
                "unlimited".to_string()
            },
            usage.disk_pressure * 100.0,
            usage.uptime_secs,
            usage.requests_total,
            usage.requests_failed,
            usage.bytes_read,
            usage.bytes_written,
        )
    }

    /// Show resource limits
    pub fn show_resource_limits(manager: &ResourceManager) -> String {
        let limits = manager.get_limits();
        
        format!(
            r#"Resource Limits:
  Max Memory: {} KB
  Max Connections: {}
  Max Databases: {}
  Max Buckets/DB: {}
  Max Keys/Bucket: {}
  Max Key Size: {} bytes
  Max Value Size: {} bytes
  Max Disk: {} MB
  Connection Idle Timeout: {} seconds
  Request Timeout: {} ms
  Monitoring: {}
  Memory Pressure Threshold: {:.0}%
  Disk Pressure Threshold: {:.0}%"#,
            if limits.max_memory_kb > 0 {
                limits.max_memory_kb.to_string()
            } else {
                "unlimited".to_string()
            },
            limits.max_connections,
            limits.max_databases,
            limits.max_buckets_per_db,
            limits.max_keys_per_bucket,
            limits.max_key_size_bytes,
            limits.max_value_size_bytes,
            if limits.max_disk_mb > 0 {
                limits.max_disk_mb.to_string()
            } else {
                "unlimited".to_string()
            },
            limits.connection_idle_timeout_secs,
            limits.request_timeout_ms,
            if limits.enable_monitoring {
                "enabled"
            } else {
                "disabled"
            },
            limits.memory_pressure_threshold * 100.0,
            limits.disk_pressure_threshold * 100.0,
        )
    }
}