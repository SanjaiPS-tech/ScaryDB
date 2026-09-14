use crate::catalog::{Catalog, DatabaseCatalog};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatabaseState {
    pub db_catalog: DatabaseCatalog,
    pub data: HashMap<u32, HashMap<u32, Value>>, // BUCKET_ID -> KEY_ID -> Value
}

pub struct StorageEngine {
    pub global_catalog: Catalog,
    pub databases: HashMap<u32, DatabaseState>,
}

impl StorageEngine {
    pub fn new() -> Self {
        StorageEngine {
            global_catalog: Catalog::default(),
            databases: HashMap::new(),
        }
    }

    // --- DDC (Database Definition Commands) ---

    pub fn create_db(&mut self, name: &str) -> Result<u32, String> {
        if self.global_catalog.get_db_id(name).is_some() {
            return Err(format!("Database '{}' already exists", name));
        }
        let db_id = self.global_catalog.get_or_create_db_id(name);
        self.databases.insert(db_id, DatabaseState::default());
        Ok(db_id)
    }

    pub fn drop_db(&mut self, name: &str) -> Result<u32, String> {
        let db_id = self.global_catalog.get_db_id(name)
            .ok_ok_or_else(|| format!("Database '{}' does not exist", name))
            .map_err(|e| e)?;
        
        self.global_catalog.remove_db(name);
        self.databases.remove(&db_id);
        Ok(db_id)
    }

    pub fn list_dbs(&self) -> Vec<String> {
        let mut dbs: Vec<String> = self.global_catalog.db_name_to_id.keys().cloned().collect();
        dbs.sort();
        dbs
    }

    pub fn create_bucket(&mut self, db_id: u32, bucket_name: &str) -> Result<u32, String> {
        let db_state = self.databases.get_mut(&db_id)
            .ok_ok_or_else(|| format!("Database with ID {} not found in storage", db_id))
            .map_err(|e| e)?;
        
        if db_state.db_catalog.get_bucket_id(bucket_name).is_some() {
            return Err(format!("Bucket '{}' already exists", bucket_name));
        }

        let bucket_id = db_state.db_catalog.get_or_create_bucket_id(bucket_name);
        db_state.data.insert(bucket_id, HashMap::new());
        Ok(bucket_id)
    }

    pub fn drop_bucket(&mut self, db_id: u32, bucket_name: &str) -> Result<u32, String> {
        let db_state = self.databases.get_mut(&db_id)
            .ok_ok_or_else(|| format!("Database with ID {} not found in storage", db_id))
            .map_err(|e| e)?;
        
        let bucket_id = db_state.db_catalog.get_bucket_id(bucket_name)
            .ok_ok_or_else(|| format!("Bucket '{}' does not exist", bucket_name))
            .map_err(|e| e)?;

        db_state.db_catalog.remove_bucket(bucket_name);
        db_state.data.remove(&bucket_id);
        Ok(bucket_id)
    }

    pub fn list_buckets(&self, db_id: u32) -> Result<Vec<String>, String> {
        let db_state = self.databases.get(&db_id)
            .ok_ok_or_else(|| format!("Database with ID {} not found in storage", db_id))
            .map_err(|e| e)?;
        
        let mut buckets: Vec<String> = db_state.db_catalog.bucket_name_to_id.keys().cloned().collect();
        buckets.sort();
        Ok(buckets)
    }

    // --- DMC (Data Manipulation Commands) ---

    pub fn set_key(&mut self, db_id: u32, bucket_name: &str, key_name: &str, value: Value) -> Result<(), String> {
        let db_state = self.databases.get_mut(&db_id)
            .ok_ok_or_else(|| format!("Database with ID {} not found in storage", db_id))
            .map_err(|e| e)?;
        
        let bucket_id = db_state.db_catalog.get_bucket_id(bucket_name)
            .ok_ok_or_else(|| format!("Bucket '{}' does not exist. Create it first.", bucket_name))
            .map_err(|e| e)?;

        let bucket_data = db_state.data.get_mut(&bucket_id)
            .ok_ok_or_else(|| format!("Bucket data storage for ID {} not initialized", bucket_id))
            .map_err(|e| e)?;

        let key_catalog = db_state.db_catalog.bucket_key_catalogs.get_mut(&bucket_id)
            .ok_ok_or_else(|| format!("Key catalog for Bucket ID {} not initialized", bucket_id))
            .map_err(|e| e)?;

        let key_id = key_catalog.get_or_create_id(key_name);
        bucket_data.insert(key_id, value);
        Ok(())
    }

    pub fn del_key(&mut self, db_id: u32, bucket_name: &str, key_name: &str) -> Result<bool, String> {
        let db_state = self.databases.get_mut(&db_id)
            .ok_ok_or_else(|| format!("Database with ID {} not found in storage", db_id))
            .map_err(|e| e)?;
        
        let bucket_id = db_state.db_catalog.get_bucket_id(bucket_name)
            .ok_ok_or_else(|| format!("Bucket '{}' does not exist", bucket_name))
            .map_err(|e| e)?;

        let bucket_data = db_state.data.get_mut(&bucket_id)
            .ok_ok_or_else(|| format!("Bucket data storage for ID {} not initialized", bucket_id))
            .map_err(|e| e)?;

        let key_catalog = db_state.db_catalog.bucket_key_catalogs.get_mut(&bucket_id)
            .ok_ok_or_else(|| format!("Key catalog for Bucket ID {} not initialized", bucket_id))
            .map_err(|e| e)?;

        if let Some(key_id) = key_catalog.remove_key(key_name) {
            bucket_data.remove(&key_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // --- DRC (Data Retrieval Commands) ---

    pub fn get_key(&self, db_id: u32, bucket_name: &str, key_name: &str) -> Result<Option<Value>, String> {
        let db_state = self.databases.get(&db_id)
            .ok_ok_or_else(|| format!("Database with ID {} not found in storage", db_id))
            .map_err(|e| e)?;
        
        let bucket_id = db_state.db_catalog.get_bucket_id(bucket_name)
            .ok_ok_or_else(|| format!("Bucket '{}' does not exist", bucket_name))
            .map_err(|e| e)?;

        let bucket_data = db_state.data.get(&bucket_id)
            .ok_ok_or_else(|| format!("Bucket data storage for ID {} not initialized", bucket_id))
            .map_err(|e| e)?;

        let key_catalog = db_state.db_catalog.bucket_key_catalogs.get(&bucket_id)
            .ok_ok_or_else(|| format!("Key catalog for Bucket ID {} not initialized", bucket_id))
            .map_err(|e| e)?;

        if let Some(key_id) = key_catalog.get_id(key_name) {
            Ok(bucket_data.get(&key_id).cloned())
        } else {
            Ok(None)
        }
    }

    pub fn exists_key(&self, db_id: u32, bucket_name: &str, key_name: &str) -> Result<bool, String> {
        let db_state = self.databases.get(&db_id)
            .ok_ok_or_else(|| format!("Database with ID {} not found in storage", db_id))
            .map_err(|e| e)?;
        
        let bucket_id = db_state.db_catalog.get_bucket_id(bucket_name)
            .ok_ok_or_else(|| format!("Bucket '{}' does not exist", bucket_name))
            .map_err(|e| e)?;

        let key_catalog = db_state.db_catalog.bucket_key_catalogs.get(&bucket_id)
            .ok_ok_or_else(|| format!("Key catalog for Bucket ID {} not initialized", bucket_id))
            .map_err(|e| e)?;

        Ok(key_catalog.get_id(key_name).is_some())
    }

    pub fn list_keys(&self, db_id: u32, bucket_name: &str) -> Result<Vec<String>, String> {
        let db_state = self.databases.get(&db_id)
            .ok_ok_or_else(|| format!("Database with ID {} not found in storage", db_id))
            .map_err(|e| e)?;
        
        let bucket_id = db_state.db_catalog.get_bucket_id(bucket_name)
            .ok_ok_or_else(|| format!("Bucket '{}' does not exist", bucket_name))
            .map_err(|e| e)?;

        let key_catalog = db_state.db_catalog.bucket_key_catalogs.get(&bucket_id)
            .ok_ok_or_else(|| format!("Key catalog for Bucket ID {} not initialized", bucket_id))
            .map_err(|e| e)?;

        let mut keys: Vec<String> = key_catalog.key_name_to_id.keys().cloned().collect();
        keys.sort();
        Ok(keys)
    }

    pub fn count_keys(&self, db_id: u32, bucket_name: &str) -> Result<usize, String> {
        let db_state = self.databases.get(&db_id)
            .ok_ok_or_else(|| format!("Database with ID {} not found in storage", db_id))
            .map_err(|e| e)?;
        
        let bucket_id = db_state.db_catalog.get_bucket_id(bucket_name)
            .ok_ok_or_else(|| format!("Bucket '{}' does not exist", bucket_name))
            .map_err(|e| e)?;

        let key_catalog = db_state.db_catalog.bucket_key_catalogs.get(&bucket_id)
            .ok_ok_or_else(|| format!("Key catalog for Bucket ID {} not initialized", bucket_id))
            .map_err(|e| e)?;

        Ok(key_catalog.key_name_to_id.len())
    }

    pub fn get_stats(&self) -> String {
        let db_count = self.databases.len();
        let mut total_buckets = 0;
        let mut total_keys = 0;
        for (_, db_state) in &self.databases {
            total_buckets += db_state.db_catalog.bucket_name_to_id.len();
            for (_, bucket_catalog) in &db_state.db_catalog.bucket_key_catalogs {
                total_keys += bucket_catalog.key_name_to_id.len();
            }
        }
        format!(
            "Databases: {}\nTotal Buckets: {}\nTotal Keys: {}",
            db_count, total_buckets, total_keys
        )
    }
}

// Custom extension trait to help with Result conversions on Option (workaround for standard traits if needed)
trait OptionExt<T> {
    fn ok_ok_or_else<F, E>(self, err: F) -> Result<T, E>
    where
        F: FnOnce() -> E;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_ok_or_else<F, E>(self, err: F) -> Result<T, E>
    where
        F: FnOnce() -> E,
    {
        self.ok_or_else(err)
    }
}
