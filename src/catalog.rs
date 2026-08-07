use ahash::AHasher;
use serde::{Deserialize, Serialize};
use smallstr::SmallString;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicU32, Ordering};

/// Fast hash map with ahash for integer keys
type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<AHasher>>;
/// Fast hash map with string keys
type FastStrMap<V> = HashMap<String, V, BuildHasherDefault<AHasher>>;

/// String interner for reducing allocations
#[derive(Debug, Default)]
pub struct StringInterner {
    strings: FastStrMap<u32>,
    reverse: FastMap<u32, String>,
    next_id: AtomicU32,
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            strings: FastStrMap::default(),
            reverse: FastMap::default(),
            next_id: AtomicU32::new(1),
        }
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.strings.get(s) {
            return id;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.strings.insert(s.to_string(), id);
        self.reverse.insert(id, s.to_string());
        id
    }

    pub fn get(&self, s: &str) -> Option<u32> {
        self.strings.get(s).copied()
    }

    pub fn resolve(&self, id: u32) -> Option<&str> {
        self.reverse.get(&id).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BucketKeyCatalog {
    pub next_key_id: u32,
    pub key_name_to_id: FastStrMap<u32>,
    pub key_id_to_name: FastMap<u32, String>,
}

impl BucketKeyCatalog {
    #[inline]
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

    #[inline]
    pub fn get_id(&self, key_name: &str) -> Option<u32> {
        self.key_name_to_id.get(key_name).copied()
    }

    #[inline]
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
    pub bucket_name_to_id: FastStrMap<u32>,
    pub bucket_id_to_name: FastMap<u32, String>,
    pub bucket_key_catalogs: FastMap<u32, BucketKeyCatalog>,
}

impl DatabaseCatalog {
    #[inline]
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

    #[inline]
    pub fn get_bucket_id(&self, bucket_name: &str) -> Option<u32> {
        self.bucket_name_to_id.get(bucket_name).copied()
    }

    #[inline]
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
    pub db_name_to_id: FastStrMap<u32>,
    pub db_id_to_name: FastMap<u32, String>,
}

impl Catalog {
    #[inline]
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

    #[inline]
    pub fn get_db_id(&self, db_name: &str) -> Option<u32> {
        self.db_name_to_id.get(db_name).copied()
    }

    #[inline]
    pub fn remove_db(&mut self, db_name: &str) -> Option<u32> {
        if let Some(id) = self.db_name_to_id.remove(db_name) {
            self.db_id_to_name.remove(&id);
            Some(id)
        } else {
            None
        }
    }
}