use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BucketKeyCatalog {
    pub next_key_id: u32,
    pub key_name_to_id: HashMap<String, u32>,
    pub key_id_to_name: HashMap<u32, String>,
}

impl BucketKeyCatalog {
    pub fn get_or_create_id(&mut self, key_name: &str) -> u32 {
        if let Some(&id) = self.key_name_to_id.get(key_name) {
            id
        } else {
            let id = self.next_key_id;
            self.next_key_id += 1;
            self.key_name_to_id.insert(key_name.to_string(), id);
            self.key_id_to_name.insert(id, key_name.to_string());
            id
        }
    }

    pub fn get_id(&self, key_name: &str) -> Option<u32> {
        self.key_name_to_id.get(key_name).copied()
    }

    pub fn remove_key(&mut self, key_name: &str) -> Option<u32> {
        if let Some(id) = self.key_name_to_id.remove(key_name) {
            self.key_id_to_name.remove(&id);
            Some(id)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatabaseCatalog {
    pub next_bucket_id: u32,
    pub bucket_name_to_id: HashMap<String, u32>,
    pub bucket_id_to_name: HashMap<u32, String>,
    pub bucket_key_catalogs: HashMap<u32, BucketKeyCatalog>,
}

impl DatabaseCatalog {
    pub fn get_or_create_bucket_id(&mut self, bucket_name: &str) -> u32 {
        if let Some(&id) = self.bucket_name_to_id.get(bucket_name) {
            id
        } else {
            let id = self.next_bucket_id;
            self.next_bucket_id += 1;
            self.bucket_name_to_id.insert(bucket_name.to_string(), id);
            self.bucket_id_to_name.insert(id, bucket_name.to_string());
            self.bucket_key_catalogs.insert(id, BucketKeyCatalog::default());
            id
        }
    }

    pub fn get_bucket_id(&self, bucket_name: &str) -> Option<u32> {
        self.bucket_name_to_id.get(bucket_name).copied()
    }

    pub fn remove_bucket(&mut self, bucket_name: &str) -> Option<u32> {
        if let Some(id) = self.bucket_name_to_id.remove(bucket_name) {
            self.bucket_id_to_name.remove(&id);
            self.bucket_key_catalogs.remove(&id);
            Some(id)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Catalog {
    pub next_db_id: u32,
    pub db_name_to_id: HashMap<String, u32>,
    pub db_id_to_name: HashMap<u32, String>,
}

impl Catalog {
    pub fn get_or_create_db_id(&mut self, db_name: &str) -> u32 {
        if let Some(&id) = self.db_name_to_id.get(db_name) {
            id
        } else {
            let id = self.next_db_id;
            self.next_db_id += 1;
            self.db_name_to_id.insert(db_name.to_string(), id);
            self.db_id_to_name.insert(id, db_name.to_string());
            id
        }
    }

    pub fn get_db_id(&self, db_name: &str) -> Option<u32> {
        self.db_name_to_id.get(db_name).copied()
    }

    pub fn remove_db(&mut self, db_name: &str) -> Option<u32> {
        if let Some(id) = self.db_name_to_id.remove(db_name) {
            self.db_id_to_name.remove(&id);
            Some(id)
        } else {
            None
        }
    }
}
