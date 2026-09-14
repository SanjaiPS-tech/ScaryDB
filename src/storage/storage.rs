//! Storage engine for ScaryDB
//!
//! Provides the core storage layer with databases, collections, documents,
//! indexes, GridFS, and transactions support.

use crate::document::{DocumentRecord, GridFSFile, IndexDef, CollectionOptions, ValidationLevel, ValidationAction, Value, bson_values_equal};
use crate::{Result, ScaryError, coll_err, doc_err, internal_err};
use bson::{Bson, Document, oid::ObjectId};
use bson::doc;
use chrono::Utc;
use dashmap::DashMap;
use parking_lot::{RwLock, Mutex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Duration, Instant};
// use tantivy::{Index, Schema, Document as TantivyDocument, IndexWriter, ReloadPolicy, TantivyError};
// use geo::{Point, Geometry, Coord};
// use rstar::{RTree, RTreeObject, AABB};
use uuid::Uuid;

/// Storage engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: String,
    pub max_memory_mb: usize,
    pub checkpoint_interval_ms: u64,
    pub wal_sync_interval_ms: u64,
    pub max_collection_size: Option<u64>,
    pub enable_compression: bool,
    pub index_cache_size: usize,
    pub text_index_enabled: bool,
    pub geo_index_enabled: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: "./data".to_string(),
            max_memory_mb: 1024,
            checkpoint_interval_ms: 60000,
            wal_sync_interval_ms: 1000,
            max_collection_size: None,
            enable_compression: true,
            index_cache_size: 10000,
            text_index_enabled: true,
            geo_index_enabled: true,
        }
    }
}

/// Database catalog entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseCatalogEntry {
    pub name: String,
    pub created_at: i64,
    pub collections: BTreeMap<String, CollectionCatalogEntry>,
    pub options: Option<Document>,
}

/// Collection catalog entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionCatalogEntry {
    pub name: String,
    pub created_at: i64,
    pub options: CollectionOptions,
    pub indexes: BTreeMap<String, IndexDef>,
    pub stats: CollectionStats,
    pub document_count: u64,
    pub data_size: u64,
    pub storage_size: u64,
    pub total_index_size: u64,
}

/// Collection statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectionStats {
    pub ns: String,
    pub count: u64,
    pub size: u64,
    pub avg_obj_size: u64,
    pub storage_size: u64,
    pub free_storage_size: u64,
    pub capped: bool,
    pub max: Option<u64>,
    pub max_size: Option<u64>,
    pub nindexes: u32,
    pub total_index_size: u64,
    pub index_sizes: BTreeMap<String, u64>,
    pub ok: bool,
}

/// Document storage with MVCC support
#[derive(Debug, Clone)]
pub struct DocumentStore {
    documents: Arc<DashMap<String, DocumentRecord>>,
    versions: Arc<DashMap<String, Vec<DocumentVersion>>>,
    deleted: Arc<DashMap<String, DeletedRecord>>,
    id_index: Arc<DashMap<String, String>>, // _id -> key
    next_sequence: AtomicU64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentVersion {
    pub version: u64,
    pub timestamp: i64,
    pub data: Document,
    pub operation: WriteOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WriteOperation {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedRecord {
    pub timestamp: i64,
    pub version: u64,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            documents: Arc::new(DashMap::new()),
            versions: Arc::new(DashMap::new()),
            deleted: Arc::new(DashMap::new()),
            id_index: Arc::new(DashMap::new()),
            next_sequence: AtomicU64::new(1),
        }
    }
    
    pub fn insert(&self, mut record: DocumentRecord) -> Result<String> {
            let id = record._id.clone();
            let key = id.to_string();
        
            // Check for duplicate
            if self.id_index.contains_key(&key) {
                return Err(doc_err!("Document with _id '{}' already exists", key));
            }

            let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
            record.data.insert("_seq", Bson::Int64(sequence as i64));
        
            self.id_index.insert(key.clone(), key.clone());
            self.documents.insert(key.clone(), record.clone());
            self.versions.insert(key.clone(), vec![DocumentVersion {
                version: 1,
                timestamp: Utc::now().timestamp_millis(),
                data: record.data.clone(),
                operation: WriteOperation::Insert,
            }]);
        
            Ok(key)
        }
    
    pub fn get(&self, id: &Value) -> Option<DocumentRecord> {
        let key = id.to_string();
        self.documents.get(&key).map(|r| r.clone())
    }
    
    pub fn update(&self, id: &Value, mut record: DocumentRecord) -> Result<()> {
        let key = id.to_string();
        
        if !self.id_index.contains_key(&key) {
            return Err(doc_err!("Document with _id '{}' not found", key));
        }
        
        let old_version = self.documents.get(&key).map(|r| r.version).unwrap_or(0);
        let new_version = old_version + 1;
        record.version = new_version;
        record.updated_at = Utc::now().timestamp_millis();
        
        // Store version history
        self.versions.entry(key.clone()).or_default().push(DocumentVersion {
            version: new_version,
            timestamp: record.updated_at,
            data: record.data.clone(),
            operation: WriteOperation::Update,
        });
        
        self.documents.insert(key, record);
        Ok(())
    }
    
    pub fn delete(&self, id: &Value) -> Result<bool> {
        let key = id.to_string();
        
        if !self.id_index.contains_key(&key) {
            return Ok(false);
        }
        
        let version = self.documents.get(&key).map(|r| r.version).unwrap_or(0);
        self.id_index.remove(&key);
        self.documents.remove(&key);
        self.deleted.insert(key.clone(), DeletedRecord {
            timestamp: Utc::now().timestamp_millis(),
            version,
        });
        Ok(true)
    }
    
    pub fn exists(&self, id: &Value) -> bool {
        let key = id.to_string();
        self.id_index.contains_key(&key)
    }
    
    pub fn find(&self, filter: &crate::document::QueryFilter) -> Vec<DocumentRecord> {
        self.documents.iter()
            .filter(|entry| filter.matches(&entry.data))
            .map(|entry| entry.value().clone())
            .collect()
    }
    
    pub fn find_with_limit(&self, filter: &crate::document::QueryFilter, limit: usize) -> Vec<DocumentRecord> {
        self.documents.iter()
            .filter(|entry| filter.matches(&entry.data))
            .take(limit)
            .map(|entry| entry.value().clone())
            .collect()
    }
    
    pub fn count(&self, filter: &crate::document::QueryFilter) -> usize {
        self.documents.iter()
            .filter(|entry| filter.matches(&entry.data))
            .count()
    }
    
    pub fn get_all_ids(&self) -> Vec<String> {
        self.id_index.iter().map(|entry| entry.key().clone()).collect()
    }
    
    pub fn len(&self) -> usize {
        self.documents.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
    
    pub fn get_version_history(&self, id: &Value) -> Vec<DocumentVersion> {
        let key = id.to_string();
        self.versions.get(&key).map(|v| v.clone()).unwrap_or_default()
    }
    
    pub fn get_deleted(&self, id: &Value) -> Option<DeletedRecord> {
        let key = id.to_string();
        self.deleted.get(&key).map(|r| r.clone())
    }
    
    pub fn estimate_memory(&self) -> usize {
        // Rough estimation
        let doc_count = self.documents.len();
        let avg_doc_size = 1024; // bytes
        doc_count * avg_doc_size
    }
}

/// Text search index using Tantivy (disabled - requires tantivy crate)
/// pub struct TextIndex {
///     index: Index,
///     writer: Mutex<IndexWriter>,
///     schema: Schema,
///     id_field: tantivy::schema::Field,
/// }
/// 
/// impl TextIndex {
///     pub fn new() -> Result<Self> {
///         let mut schema_builder = Schema::builder();
///         let id_field = schema_builder.add_text_field("_id", tantivy::schema::STRING | tantivy::schema::STORED);
///         let content_field = schema_builder.add_text_field("content", tantivy::schema::TEXT);
///         let schema = schema_builder.build();
///         
///         let index = Index::create_in_ram(schema.clone());
///         let writer = index.writer(50_000_000)?; // 50MB buffer
///         
///         Ok(Self {
///             index,
///             writer: Mutex::new(writer),
///             schema,
///             id_field,
///         })
///     }
///     
///     pub fn add_document(&self, id: &str, content: &str) -> Result<()> {
///         let mut doc = TantivyDocument::default();
///         doc.add_text(self.id_field, id);
///         doc.add_text(self.schema.get_field("content").unwrap(), content);
///         
///         let mut writer = self.writer.lock();
///         writer.add_document(doc)?;
///         writer.commit()?;
///         Ok(())
///     }
///     
///     pub fn remove_document(&self, id: &str) -> Result<()> {
///         let mut writer = self.writer.lock();
///         writer.delete_term(tantivy::Term::from_field_text(self.id_field, id));
///         writer.commit()?;
///         Ok(())
///     }
///     
///     pub fn search(&self, query: &str, limit: usize) -> Result<Vec<String>> {
///         let reader = self.index.reader_builder()
///             .reload_policy(ReloadPolicy::OnCommitWithDelay(Duration::from_millis(100)))
///             .try_into()?;
///         
///         let searcher = reader.searcher();
///         let query_parser = tantivy::query::QueryParser::for_index(&self.index, vec![self.schema.get_field("content").unwrap()]);
///         let query = query_parser.parse_query(query)?;
///         
///         let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(limit))?;
///         
///         let mut results = Vec::new();
///         for (_score, doc_address) in top_docs {
///             let retrieved_doc = searcher.doc(doc_address)?;
///             if let Some(id_field) = self.schema.get_field("_id") {
///                 if let Some(id_value) = retrieved_doc.get_first(id_field) {
///                     results.push(id_value.as_text().unwrap_or("").to_string());
///                 }
///             }
///         }
///         
///         Ok(results)
///     }
/// }

/// Geospatial index using R-tree (disabled - requires geo and rstar crates)
/// pub struct GeoIndex {
///     tree: Mutex<RTree<GeoEntry>>,
/// }
/// 
/// #[derive(Debug, Clone)]
/// pub struct GeoEntry {
///     pub id: String,
///     pub geometry: Geometry<f64>,
///     pub bbox: AABB<[f64; 2]>,
/// }
/// 
/// impl RTreeObject for GeoEntry {
///     type Envelope = AABB<[f64; 2]>;
///     
///     fn envelope(&self) -> Self::Envelope {
///         self.bbox
///     }
/// }
/// 
/// impl GeoIndex {
///     pub fn new() -> Self {
///         Self {
///             tree: Mutex::new(RTree::new()),
///         }
///     }
    
    ///     pub fn insert(&self, id: String, geometry: Geometry<f64>) {
///         let bbox = geometry.bounding_box().unwrap_or_else(|| AABB::from_corners([0.0, 0.0], [0.0, 0.0]));
///         let entry = GeoEntry { id, geometry, bbox };
///         self.tree.lock().insert(entry);
///     }
///     
///     pub fn remove(&self, id: &str) -> bool {
///         // RTree doesn't support direct removal by ID easily
///         // In production, would use a secondary index
///         false
///     }
///     
///     pub fn search_within(&self, geometry: &Geometry<f64>) -> Vec<String> {
///         let bbox = geometry.bounding_box().unwrap_or_else(|| AABB::from_corners([0.0, 0.0], [0.0, 0.0]));
///         let tree = self.tree.lock();
///         tree.locate_in_envelope_intersecting(&bbox)
///             .filter(|entry| entry.geometry.intersects(geometry))
///             .map(|entry| entry.id.clone())
///             .collect()
///     }
///     
///     pub fn search_near(&self, point: Point<f64>, distance: f64, limit: usize) -> Vec<String> {
///         let tree = self.tree.lock();
///         tree.nearest_neighbor_iter(&point)
///             .take(limit)
///             .filter(|entry| entry.geometry.distance(point) <= distance)
///             .map(|entry| entry.id.clone())
///             .collect()
///     }
/// }

/// B-tree index for exact match and range queries
pub struct BTreeIndex {
    index: Arc<DashMap<Bson, BTreeSet<String>>>,
    unique: bool,
    field_name: String,
}

impl BTreeIndex {
    pub fn new(field_name: String, unique: bool) -> Self {
        Self {
            index: Arc::new(DashMap::new()),
            unique,
            field_name,
        }
    }
    
    pub fn insert(&self, value: Bson, doc_id: String) -> Result<()> {
        let mut entry = self.index.entry(value).or_default();
        if self.unique && !entry.is_empty() {
            return Err(coll_err!("Duplicate key error on field '{}'", self.field_name));
        }
        entry.insert(doc_id);
        Ok(())
    }
    
    pub fn remove(&self, value: &Bson, doc_id: &str) -> bool {
        if let Some(mut entry) = self.index.get_mut(value) {
            entry.remove(doc_id);
            if entry.is_empty() {
                drop(entry);
                self.index.remove(value);
            }
            true
        } else {
            false
        }
    }
    
    pub fn find_exact(&self, value: &Bson) -> Vec<String> {
        self.index.get(value).map(|e| e.iter().cloned().collect()).unwrap_or_default()
    }
    
    pub fn find_range(&self, min: Option<&Bson>, max: Option<&Bson>, include_min: bool, include_max: bool) -> Vec<String> {
        // Simplified - in production would use proper B-tree range queries
        let mut results = Vec::new();
        for entry in self.index.iter() {
            let key = entry.key();
            let in_range = match (min, max) {
                (Some(min), Some(max)) => {
                    let cmp_min = bson_compare(key, min);
                    let cmp_max = bson_compare(key, max);
                    (include_min && cmp_min >= 0 || !include_min && cmp_min > 0) &&
                    (include_max && cmp_max <= 0 || !include_max && cmp_max < 0)
                }
                (Some(min), None) => {
                    let cmp = bson_compare(key, min);
                    include_min && cmp >= 0 || !include_min && cmp > 0
                }
                (None, Some(max)) => {
                    let cmp = bson_compare(key, max);
                    include_max && cmp <= 0 || !include_max && cmp < 0
                }
                (None, None) => true,
            };
            if in_range {
                results.extend(entry.value().iter().cloned());
            }
        }
        results
    }
    
    pub fn len(&self) -> usize {
        self.index.len()
    }
}

fn bson_compare(a: &Bson, b: &Bson) -> i32 {
    // Simplified comparison
    match (a, b) {
        (Bson::Int32(a), Bson::Int32(b)) => a.cmp(b) as i32,
        (Bson::Int64(a), Bson::Int64(b)) => a.cmp(b) as i32,
        (Bson::Double(a), Bson::Double(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal) as i32,
        (Bson::String(a), Bson::String(b)) => a.cmp(b) as i32,
        _ => 0,
    }
}

/// Compound index for multi-field queries
pub struct CompoundIndex {
    fields: Vec<String>,
    index: Arc<DashMap<Vec<Bson>, BTreeSet<String>>>,
    unique: bool,
}

impl CompoundIndex {
    pub fn new(fields: Vec<String>, unique: bool) -> Self {
        Self {
            fields,
            index: Arc::new(DashMap::new()),
            unique,
        }
    }
    
    pub fn make_key(&self, doc: &Document) -> Option<Vec<Bson>> {
        let mut key = Vec::new();
        for field in &self.fields {
            if let Some(val) = doc.get(field) {
                key.push(val.clone());
            } else {
                return None;
            }
        }
        Some(key)
    }
    
    pub fn insert(&self, doc: &Document, doc_id: String) -> Result<()> {
        if let Some(key) = self.make_key(doc) {
            let mut entry = self.index.entry(key).or_default();
            if self.unique && !entry.is_empty() {
                return Err(coll_err!("Duplicate key error on compound index"));
            }
            entry.insert(doc_id);
        }
        Ok(())
    }
    
    pub fn remove(&self, doc: &Document, doc_id: &str) -> bool {
        if let Some(key) = self.make_key(doc) {
            if let Some(mut entry) = self.index.get_mut(&key) {
                entry.remove(doc_id);
                if entry.is_empty() {
                    drop(entry);
                    self.index.remove(&key);
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    
    pub fn find(&self, key: &[Bson]) -> Vec<String> {
        self.index.get(key).map(|e| e.iter().cloned().collect()).unwrap_or_default()
    }
}

/// GridFS storage for large files
pub struct GridFSStore {
    files: Arc<DashMap<String, GridFSFile>>,
    chunks: Arc<DashMap<String, Vec<u8>>>, // chunk_id -> data
    chunk_size: i32,
}

impl GridFSStore {
    pub fn new(chunk_size: i32) -> Self {
        Self {
            files: Arc::new(DashMap::new()),
            chunks: Arc::new(DashMap::new()),
            chunk_size,
        }
    }
    
    pub fn create_file(&self, filename: String, content_type: Option<String>, metadata: Option<Document>) -> Result<GridFSFile> {
        let id = Value::object_id(ObjectId::new().to_hex());
        let file = GridFSFile {
            _id: id.clone(),
            length: 0,
            chunk_size: self.chunk_size,
            upload_date: Utc::now().timestamp_millis(),
            md5: None,
            filename,
            content_type,
            aliases: None,
            metadata,
        };
        self.files.insert(id.to_string(), file.clone());
        Ok(file)
    }
    
    pub fn write_chunk(&self, file_id: &str, chunk_number: i32, data: Vec<u8>) -> Result<()> {
        let chunk_id = format!("{}_{}", file_id, chunk_number);
        let data_len = data.len();
        self.chunks.insert(chunk_id, data);
        
        // Update file length
        if let Some(mut file) = self.files.get_mut(file_id) {
            file.length += data_len as i64;
        }
        Ok(())
    }
    
    pub fn read_chunk(&self, file_id: &str, chunk_number: i32) -> Option<Vec<u8>> {
        let chunk_id = format!("{}_{}", file_id, chunk_number);
        self.chunks.get(&chunk_id).map(|c| c.clone())
    }
    
    pub fn get_file(&self, file_id: &str) -> Option<GridFSFile> {
        self.files.get(file_id).map(|f| f.clone())
    }
    
    pub fn delete_file(&self, file_id: &str) -> Result<bool> {
        if !self.files.contains_key(file_id) {
            return Ok(false);
        }
        
        // Delete all chunks
        let mut chunk_num = 0;
        loop {
            let chunk_id = format!("{}_{}", file_id, chunk_num);
            if self.chunks.remove(&chunk_id).is_none() {
                break;
            }
            chunk_num += 1;
        }
        
        self.files.remove(file_id);
        Ok(true)
    }
    
    pub fn list_files(&self, filter: Option<&crate::document::QueryFilter>) -> Vec<GridFSFile> {
        self.files.iter()
            .filter(|entry| filter.map_or(true, |f| f.matches(&entry.value().to_document())))
            .map(|entry| entry.value().clone())
            .collect()
    }
}

fn document_to_bson(doc: &DocumentRecord) -> Document {
    doc.to_document()
}

/// Main storage engine
pub struct StorageEngine {
    config: StorageConfig,
    databases: Arc<DashMap<String, Arc<RwLock<Database>>>>,
    global_stats: Arc<RwLock<GlobalStats>>,
    // text_index and geo_index are disabled (require additional crates)
    wal: Arc<WAL>,
    checkpoint_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    wal_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    shutdown: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalStats {
    pub total_databases: usize,
    pub total_collections: usize,
    pub total_documents: u64,
    pub total_data_size: u64,
    pub total_index_size: u64,
    pub uptime_seconds: u64,
    pub start_time: i64,
}

/// Write-ahead log
pub struct WAL {
    path: String,
    writer: Mutex<std::fs::File>,
    buffer: Mutex<Vec<WALEntry>>,
    sync_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WALEntry {
    pub sequence: u64,
    pub timestamp: i64,
    pub operation: WALOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WALOperation {
    Insert { db: String, collection: String, document: DocumentRecord },
    Update { db: String, collection: String, id: String, document: DocumentRecord },
    Delete { db: String, collection: String, id: String },
    CreateCollection { db: String, collection: String, options: CollectionOptions },
    DropCollection { db: String, collection: String },
    CreateIndex { db: String, collection: String, index: IndexDef },
    DropIndex { db: String, collection: String, index_name: String },
    CreateDatabase { name: String },
    DropDatabase { name: String },
    TransactionBegin { txn_id: String },
    TransactionCommit { txn_id: String },
    TransactionAbort { txn_id: String },
}

impl WAL {
    pub fn new(path: String, sync_interval: Duration) -> Result<Self> {
        std::fs::create_dir_all(&path).map_err(|e| ScaryError::Io(e))?;
        let wal_path = format!("{}/wal.log", path);
        let writer = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wal_path)
            .map_err(|e| ScaryError::Io(e))?;
        
        Ok(Self {
            path: wal_path,
            writer: Mutex::new(writer),
            buffer: Mutex::new(Vec::new()),
            sync_interval,
        })
    }
    
    pub fn write(&self, entry: WALEntry) -> Result<()> {
        let mut buffer = self.buffer.lock();
        buffer.push(entry);
        
        if buffer.len() >= 100 {
            self.flush_buffer(&mut buffer)?;
        }
        Ok(())
    }
    
    fn flush_buffer(&self, buffer: &mut Vec<WALEntry>) -> Result<()> {
        if buffer.is_empty() {
            return Ok(());
        }
        
        let mut writer = self.writer.lock();
        for entry in buffer.drain(..) {
            let data = bson::to_vec(&entry).map_err(|e| ScaryError::Bson(e))?;
            let len = (data.len() as u32).to_be_bytes();
            writer.write_all(&len).map_err(|e| ScaryError::Io(e))?;
            writer.write_all(&data).map_err(|e| ScaryError::Io(e))?;
        }
        writer.flush().map_err(|e| ScaryError::Io(e))?;
        Ok(())
    }
    
    pub fn flush(&self) -> Result<()> {
        let mut buffer = self.buffer.lock();
        self.flush_buffer(&mut buffer)
    }
    
    pub fn replay(&self) -> Result<Vec<WALEntry>> {
        // Implementation for WAL replay on startup
        Ok(Vec::new())
    }
}

/// Database instance
pub struct Database {
    name: String,
    collections: Arc<DashMap<String, Arc<RwLock<Collection>>>>,
    options: Option<Document>,
    created_at: i64,
}

impl Database {
    pub fn new(name: String, options: Option<Document>) -> Self {
        Self {
            name,
            collections: Arc::new(DashMap::new()),
            options,
            created_at: Utc::now().timestamp_millis(),
        }
    }
    
    pub fn name(&self) -> &str {
        &self.name
    }
    
    pub fn create_collection(&self, name: String, options: CollectionOptions) -> Result<Arc<RwLock<Collection>>> {
        if self.collections.contains_key(&name) {
            return Err(coll_err!("Collection '{}' already exists", name));
        }
        
        let collection = Arc::new(RwLock::new(Collection::new(name.clone(), options)));
        self.collections.insert(name, collection.clone());
        Ok(collection)
    }
    
    pub fn get_collection(&self, name: &str) -> Option<Arc<RwLock<Collection>>> {
        self.collections.get(name).map(|c| c.clone())
    }
    
    pub fn drop_collection(&self, name: &str) -> Result<bool> {
        Ok(self.collections.remove(name).is_some())
    }
    
    pub fn list_collections(&self) -> Vec<String> {
        self.collections.iter().map(|c| c.key().clone()).collect()
    }
    
    pub fn collection_count(&self) -> usize {
        self.collections.len()
    }
}

/// Collection instance
pub struct Collection {
    name: String,
    options: CollectionOptions,
    store: DocumentStore,
    indexes: Arc<DashMap<String, Box<dyn IndexTrait>>>,
    stats: Arc<RwLock<CollectionStats>>,
    created_at: i64,
}

impl Collection {
    pub fn new(name: String, options: CollectionOptions) -> Self {
        Self {
            name,
            options,
            store: DocumentStore::new(),
            indexes: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(CollectionStats::default())),
            created_at: Utc::now().timestamp_millis(),
        }
    }
    
    pub fn name(&self) -> &str {
        &self.name
    }
    
    pub fn insert(&self, mut record: DocumentRecord) -> Result<String> {
        // Validate document
        self.validate_document(&record.data)?;
        
        let id = self.store.insert(record)?;
        self.update_stats_insert();
        Ok(id)
    }
    
    pub fn insert_many(&self, records: Vec<DocumentRecord>) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for record in records {
            ids.push(self.insert(record)?);
        }
        Ok(ids)
    }
    
    pub fn find_one(&self, filter: &crate::document::QueryFilter) -> Option<DocumentRecord> {
        self.store.find_with_limit(filter, 1).into_iter().next()
    }
    
    pub fn find(&self, filter: &crate::document::QueryFilter, limit: Option<usize>) -> Vec<DocumentRecord> {
        if let Some(lim) = limit {
            self.store.find_with_limit(filter, lim)
        } else {
            self.store.find(filter)
        }
    }
    
    pub fn find_by_id(&self, id: &Value) -> Option<DocumentRecord> {
        self.store.get(id)
    }
    
    pub fn update_one(&self, filter: &crate::document::QueryFilter, update: &crate::document::UpdateSpec) -> Result<bool> {
            let docs = self.store.find_with_limit(filter, 1);
            if let Some(mut doc) = docs.into_iter().next() {
                let id = doc._id.clone();
                self.apply_update(&mut doc, update)?;
                self.store.update(&id, doc)?;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        pub fn update_many(&self, filter: &crate::document::QueryFilter, update: &crate::document::UpdateSpec) -> Result<usize> {
            let docs = self.store.find(filter);
            let mut count = 0;
            for mut doc in docs {
                let id = doc._id.clone();
                self.apply_update(&mut doc, update)?;
                self.store.update(&id, doc)?;
                count += 1;
            }
            Ok(count)
        }

        pub fn replace_one(&self, filter: &crate::document::QueryFilter, replacement: DocumentRecord) -> Result<bool> {
            let docs = self.store.find_with_limit(filter, 1);
            if let Some(mut doc) = docs.into_iter().next() {
                let id = doc._id.clone();
                doc.data = replacement.data;
                self.validate_document(&doc.data)?;
                self.store.update(&id, doc)?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    
    pub fn delete_one(&self, filter: &crate::document::QueryFilter) -> Result<bool> {
        let docs = self.store.find_with_limit(filter, 1);
        if let Some(doc) = docs.into_iter().next() {
            self.store.delete(&doc._id)?;
            self.update_stats_delete();
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    pub fn delete_many(&self, filter: &crate::document::QueryFilter) -> Result<usize> {
        let docs = self.store.find(filter);
        let count = docs.len();
        for doc in docs {
            self.store.delete(&doc._id)?;
        }
        self.update_stats_delete_many(count);
        Ok(count)
    }
    
    pub fn count(&self, filter: &crate::document::QueryFilter) -> usize {
        self.store.count(filter)
    }
    
    pub fn create_index(&self, index: IndexDef) -> Result<()> {
        if self.indexes.contains_key(&index.name) {
            return Err(coll_err!("Index '{}' already exists", index.name));
        }
        
        let index_impl: Box<dyn IndexTrait> = match index.keys.len() {
            1 => {
                let (field, _) = index.keys.iter().next().unwrap();
                Box::new(BTreeIndex::new(field.clone(), index.unique))
            }
            _ => Box::new(CompoundIndex::new(index.keys.keys().cloned().collect(), index.unique)),
        };
        
        // Build index from existing documents
        for entry in self.store.documents.iter() {
            index_impl.insert(&entry.data, entry._id.to_string())?;
        }
        
        self.indexes.insert(index.name.clone(), index_impl);
        
        // Update catalog
        let mut stats = self.stats.write();
        stats.nindexes += 1;
        
        Ok(())
    }
    
    pub fn drop_index(&self, name: &str) -> Result<bool> {
        Ok(self.indexes.remove(name).is_some())
    }
    
    pub fn list_indexes(&self) -> Vec<String> {
        self.indexes.iter().map(|i| i.key().clone()).collect()
    }
    
    fn validate_document(&self, doc: &Document) -> Result<()> {
        if let Some(validator) = &self.options.validator {
            // Simplified validation - in production would use JSON Schema
            // For now, just check required fields
            if let Some(required) = validator.get("required") {
                if let Bson::Array(req_fields) = required {
                    for field in req_fields {
                        if let Bson::String(field_name) = field {
                            if !doc.contains_key(field_name) {
                                return Err(doc_err!("Required field '{}' missing", field_name));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    fn apply_update(&self, record: &mut DocumentRecord, update: &crate::document::UpdateSpec) -> Result<()> {
        let data = record.get_data_mut();
        
        for (op, value) in &update.operations {
            match op.as_str() {
                "$set" => {
                    if let Bson::Document(set_doc) = value {
                        for (k, v) in set_doc {
                            data.insert(k, v.clone());
                        }
                    }
                }
                "$unset" => {
                    if let Bson::Document(unset_doc) = value {
                        for k in unset_doc.keys() {
                            data.remove(k);
                        }
                    }
                }
                "$inc" => {
                    if let Bson::Document(inc_doc) = value {
                        for (k, v) in inc_doc {
                            if let Some(Bson::Int64(current)) = data.get(k) {
                                if let Bson::Int64(delta) = v {
                                    data.insert(k, Bson::Int64(current + delta));
                                }
                            } else if let Some(Bson::Int32(current)) = data.get(k) {
                                if let Bson::Int64(delta) = v {
                                    data.insert(k, Bson::Int64(current as i64 + delta));
                                }
                            } else if let Some(Bson::Double(current)) = data.get(k) {
                                if let Bson::Double(delta) = v {
                                    data.insert(k, Bson::Double(current + delta));
                                }
                            }
                        }
                    }
                }
                "$push" => {
                    if let Bson::Document(push_doc) = value {
                        if let Some(Bson::Array(each)) = push_doc.get("$each") {
                            for (field_name, field_value) in push_doc {
                                if field_name == "$each" { continue; }
                                // Handle modifiers like $slice, $sort, $position
                            }
                            for v in each {
                                // Need to determine field name - for $each it applies to the field being pushed to
                                // This is a simplified implementation
                            }
                        } else {
                            for (k, v) in push_doc {
                                match data.get_mut(k) {
                                    Some(Bson::Array(arr)) => arr.push(v.clone()),
                                    _ => { data.insert(k.clone(), Bson::Array(vec![v.clone()])); }
                                }
                            }
                        }
                    }
                }
                "$addToSet" => {
                    if let Bson::Document(set_doc) = value {
                        if let Some(Bson::Array(each)) = set_doc.get("$each") {
                            // $each with $addToSet - need to determine which field this applies to
                            // This is a simplified implementation
                            for v in each {
                                // Need to know the field name
                            }
                        } else {
                            for (k, v) in set_doc {
                                match data.get_mut(k) {
                                    Some(Bson::Array(arr)) => {
                                        if !arr.iter().any(|x| bson_values_equal(x, v)) {
                                            arr.push(v.clone());
                                        }
                                    }
                                    _ => { data.insert(k.clone(), Bson::Array(vec![v.clone()])); }
                                }
                            }
                        }
                    }
                }
                "$pull" => {
                    if let Bson::Document(pull_doc) = value {
                        for (k, v) in pull_doc {
                            if let Some(Bson::Array(arr)) = data.get_mut(k) {
                                arr.retain(|x| !bson_values_equal(x, v));
                            }
                        }
                    }
                }
                "$pullAll" => {
                    if let Bson::Document(pull_doc) = value {
                        for (k, v) in pull_doc {
                            if let Bson::Array(pull_values) = v {
                                if let Some(Bson::Array(arr)) = data.get_mut(k) {
                                    arr.retain(|x| !pull_values.iter().any(|pv| bson_values_equal(x, pv)));
                                }
                            }
                        }
                    }
                }
                _ => {} // Ignore unknown operators
            }
        }
        Ok(())
    }
    
    fn update_stats_insert(&self) {
        let mut stats = self.stats.write();
        stats.count += 1;
        stats.size += 1024; // Estimated
    }
    
    fn update_stats_delete(&self) {
        let mut stats = self.stats.write();
        stats.count = stats.count.saturating_sub(1);
    }
    
    fn update_stats_delete_many(&self, count: usize) {
        let mut stats = self.stats.write();
        stats.count = stats.count.saturating_sub(count as u64);
    }
    
    pub fn get_stats(&self) -> CollectionStats {
        self.stats.read().clone()
    }
    
    pub fn estimated_document_count(&self) -> u64 {
        self.store.len() as u64
    }
}

/// Index trait for polymorphism
pub trait IndexTrait: Send + Sync {
    fn insert(&self, doc: &Document, doc_id: String) -> Result<()>;
    fn remove(&self, doc: &Document, doc_id: &str) -> bool;
    fn as_any(&self) -> &dyn std::any::Any;
}

impl IndexTrait for BTreeIndex {
    fn insert(&self, doc: &Document, doc_id: String) -> Result<()> {
        // Find the field value
        for entry in doc.iter() {
            self.insert(entry.1.clone(), doc_id.clone())?;
        }
        Ok(())
    }
    
    fn remove(&self, doc: &Document, doc_id: &str) -> bool {
        for entry in doc.iter() {
            self.remove(entry.1, doc_id);
        }
        true
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl IndexTrait for CompoundIndex {
    fn insert(&self, doc: &Document, doc_id: String) -> Result<()> {
        self.insert(doc, doc_id)
    }
    
    fn remove(&self, doc: &Document, doc_id: &str) -> bool {
        self.remove(doc, doc_id)
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl StorageEngine {
    pub fn new(config: StorageConfig) -> Result<Self> {
        let wal = Arc::new(WAL::new(config.data_dir.clone(), Duration::from_millis(config.wal_sync_interval_ms))?);
        
        // Text index and geo index are disabled (require additional crates)
        let text_index = None;
        let geo_index = None;
        
        let engine = Self {
            config,
            databases: Arc::new(DashMap::new()),
            global_stats: Arc::new(RwLock::new(GlobalStats {
                start_time: Utc::now().timestamp_millis(),
                ..Default::default()
            })),
            text_index,
            geo_index,
            wal,
            checkpoint_handle: Mutex::new(None),
            wal_handle: Mutex::new(None),
            shutdown: Arc::new(AtomicBool::new(false)),
        };
        
        engine.start_background_tasks();
        Ok(engine)
    }
    
    fn start_background_tasks(&self) {
        // Checkpoint task
        let checkpoint_interval = Duration::from_millis(self.config.checkpoint_interval_ms);
        let shutdown = self.shutdown.clone();
        let wal = self.wal.clone();
        
        let handle = std::thread::spawn(move || {
            loop {
                std::thread::sleep(checkpoint_interval);
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                // Perform checkpoint
                if let Err(e) = wal.flush() {
                    eprintln!("WAL flush failed: {}", e);
                }
            }
        });
        self.checkpoint_handle.lock().replace(handle);
        
        // WAL sync task
        let wal_sync_interval = Duration::from_millis(self.config.wal_sync_interval_ms);
        let shutdown = self.shutdown.clone();
        let wal = self.wal.clone();
        
        let handle = std::thread::spawn(move || {
            loop {
                std::thread::sleep(wal_sync_interval);
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                if let Err(e) = wal.flush() {
                    eprintln!("WAL flush failed: {}", e);
                }
            }
        });
        self.wal_handle.lock().replace(handle);
    }
    
    pub fn create_database(&self, name: String, options: Option<Document>) -> Result<()> {
        if self.databases.contains_key(&name) {
            return Err(coll_err!("Database '{}' already exists", name));
        }

        let db = Arc::new(RwLock::new(Database::new(name.clone(), options)));
        self.databases.insert(name.clone(), db);

        let mut stats = self.global_stats.write();
        stats.total_databases += 1;

        self.wal.write(WALEntry {
            sequence: 0, // Would use proper sequence
            timestamp: Utc::now().timestamp_millis(),
            operation: WALOperation::CreateDatabase { name: name.clone() },
        })?;

        Ok(())
    }
    
    pub fn drop_database(&self, name: &str) -> Result<bool> {
        if !self.databases.contains_key(name) {
            return Ok(false);
        }
        
        self.databases.remove(name);
        
        let mut stats = self.global_stats.write();
        stats.total_databases = stats.total_databases.saturating_sub(1);
        
        self.wal.write(WALEntry {
            sequence: 0,
            timestamp: Utc::now().timestamp_millis(),
            operation: WALOperation::DropDatabase { name: name.to_string() },
        })?;
        
        Ok(true)
    }
    
    pub fn get_database(&self, name: &str) -> Option<Arc<RwLock<Database>>> {
        self.databases.get(name).map(|d| d.clone())
    }
    
    pub fn list_databases(&self) -> Vec<String> {
        self.databases.iter().map(|d| d.key().clone()).collect()
    }
    
    pub fn get_global_stats(&self) -> GlobalStats {
        let mut stats = self.global_stats.read().clone();
        stats.uptime_seconds = (Utc::now().timestamp_millis() - stats.start_time) / 1000;
        stats
    }
    
    pub fn shutdown(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::Relaxed);
        
        // Wait for background tasks
        if let Some(handle) = self.checkpoint_handle.lock().take() {
            handle.join().ok();
        }
        if let Some(handle) = self.wal_handle.lock().take() {
            handle.join().ok();
        }
        
        // Final WAL flush
        self.wal.flush()?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::*;
    use bson::doc;
    
    #[test]
    fn test_document_store() {
        let store = DocumentStore::new();
        let record = DocumentRecord::new(doc! {"name": "test", "value": 42});
        let id = store.insert(record.clone()).unwrap();
        assert!(store.exists(&record._id));
        
        let retrieved = store.get(&record._id).unwrap();
        assert_eq!(retrieved.data.get_str("name").unwrap(), "test");
    }
    
    #[test]
    fn test_btree_index() {
        let index = BTreeIndex::new("name".to_string(), true);
        index.insert(Bson::String("test".to_string()), "doc1".to_string()).unwrap();
        
        let results = index.find_exact(&Bson::String("test".to_string()));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "doc1");
    }
    
    #[test]
    fn test_gridfs() {
        let gridfs = GridFSStore::new(255);
        let file = gridfs.create_file("test.txt".to_string(), Some("text/plain".to_string()), None).unwrap();
        
        gridfs.write_chunk(&file._id.to_string(), 0, b"Hello".to_vec()).unwrap();
        gridfs.write_chunk(&file._id.to_string(), 1, b" World".to_vec()).unwrap();
        
        let chunk0 = gridfs.read_chunk(&file._id.to_string(), 0).unwrap();
        assert_eq!(chunk0, b"Hello");
        
        let chunk1 = gridfs.read_chunk(&file._id.to_string(), 1).unwrap();
        assert_eq!(chunk1, b" World");
    }
}