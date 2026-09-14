//! Error handling module for ScaryDB
//!
//! Provides comprehensive error types with context for debugging and user-friendly messages.

use anyhow::Context;
use bson::Document;
use std::fmt;
use thiserror::Error;

// Re-export bson's doc macro for convenience
use bson::doc;

/// Main error type for ScaryDB operations
#[derive(Error, Debug)]
pub enum ScaryError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Collection error: {0}")]
    Collection(String),
    
    #[error("Document error: {0}")]
    Document(String),
    
    #[error("Index error: {0}")]
    Index(String),
    
    #[error("Query error: {0}")]
    Query(String),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    
    #[error("Persistence error: {0}")]
    Persistence(String),
    
    #[error("WAL error: {0}")]
    Wal(String),
    
    #[error("Checkpoint error: {0}")]
    Checkpoint(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Lock error: {0}")]
    Lock(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Authorization error: {0}")]
    Authorization(String),
    
    #[error("TLS error: {0}")]
    Tls(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("File format error: {0}")]
    FileFormat(String),
    
    #[error("Import error: {0}")]
    Import(String),
    
    #[error("Export error: {0}")]
    Export(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("BSON error: {0}")]
    Bson(#[from] bson::ser::Error),
    
    #[error("BSON deserialization error: {0}")]
    BsonDe(#[from] bson::de::Error),
    
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::DeError),
    
    #[error("YAML error: {0}")]
    Yaml(#[from] yaml_rust2::ScanError),
    
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
    
    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),
    
    #[error("Tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
    
    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),
    
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    
    #[error("Bcrypt error: {0}")]
    Bcrypt(#[from] bcrypt::BcryptError),
    
    #[error("Rustls error: {0}")]
        Rustls(#[from] rustls::Error),

        #[error("Config error: {0}")]
    ConfigParse(#[from] config::ConfigError),
    
    #[error("Walkdir error: {0}")]
    Walkdir(#[from] walkdir::Error),
    
    #[error("Glob error: {0}")]
    Glob(#[from] glob::GlobError),
    
    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),
    
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    
    #[error("String conversion error: {0}")]
    FromUtf8(#[from] std::string::FromUtf8Error),
    
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Already exists: {0}")]
    AlreadyExists(String),
    
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    
    #[error("Operation not supported: {0}")]
    NotSupported(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),
    
    #[error("Aggregation error: {0}")]
    Aggregation(String),
    
    #[error("GridFS error: {0}")]
    GridFS(String),
}

/// Result type alias for ScaryDB operations
pub type Result<T> = std::result::Result<T, ScaryError>;

/// Extension trait for adding context to errors
pub trait ScaryResultExt<T> {
    fn with_db_context(self, db_name: &str) -> Result<T>;
    fn with_collection_context(self, collection: &str) -> Result<T>;
    fn with_document_context(self, doc_id: &str) -> Result<T>;
    fn with_query_context(self, query: &str) -> Result<T>;
    fn with_file_context(self, path: &str) -> Result<T>;
}

impl<T, E> ScaryResultExt<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_db_context(self, db_name: &str) -> Result<T> {
        self.map_err(|e| ScaryError::Database(format!("db '{}': {}", db_name, e)))
    }
    
    fn with_collection_context(self, collection: &str) -> Result<T> {
        self.map_err(|e| ScaryError::Collection(format!("collection '{}': {}", collection, e)))
    }
    
    fn with_document_context(self, doc_id: &str) -> Result<T> {
        self.map_err(|e| ScaryError::Document(format!("document '{}': {}", doc_id, e)))
    }
    
    fn with_query_context(self, query: &str) -> Result<T> {
        self.map_err(|e| ScaryError::Query(format!("query '{}': {}", query, e)))
    }
    
    fn with_file_context(self, path: &str) -> Result<T> {
        self.map_err(|e| ScaryError::FileFormat(format!("file '{}': {}", path, e)))
    }
}

/// Error response for wire protocol
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub details: Option<Document>,
}

impl ErrorResponse {
    pub fn from_error(err: &ScaryError) -> Self {
        let (code, details) = match err {
            ScaryError::NotFound(msg) => ("NOT_FOUND", Some(doc! {"message": msg})),
            ScaryError::AlreadyExists(msg) => ("ALREADY_EXISTS", Some(doc! {"message": msg})),
            ScaryError::InvalidArgument(msg) => ("INVALID_ARGUMENT", Some(doc! {"message": msg})),
            ScaryError::Auth(msg) => ("UNAUTHENTICATED", Some(doc! {"message": msg})),
            ScaryError::Authorization(msg) => ("PERMISSION_DENIED", Some(doc! {"message": msg})),
            ScaryError::NotSupported(msg) => ("NOT_SUPPORTED", Some(doc! {"message": msg})),
            ScaryError::Timeout(msg) => ("TIMEOUT", Some(doc! {"message": msg})),
            ScaryError::ResourceExhausted(msg) => ("RESOURCE_EXHAUSTED", Some(doc! {"message": msg})),
            _ => ("INTERNAL", None),
        };
        
        Self {
            error: err.to_string(),
            code: code.to_string(),
            details,
        }
    }
}

/// Convenience macros for error creation
#[macro_export]
macro_rules! db_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::Database(format!($($arg)*)) };
}

#[macro_export]
macro_rules! coll_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::Collection(format!($($arg)*)) };
}

#[macro_export]
macro_rules! doc_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::Document(format!($($arg)*)) };
}

#[macro_export]
macro_rules! query_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::Query(format!($($arg)*)) };
}

#[macro_export]
macro_rules! persist_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::Persistence(format!($($arg)*)) };
}

#[macro_export]
macro_rules! wal_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::Wal(format!($($arg)*)) };
}

#[macro_export]
macro_rules! import_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::Import(format!($($arg)*)) };
}

#[macro_export]
macro_rules! export_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::Export(format!($($arg)*)) };
}

#[macro_export]
macro_rules! format_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::FileFormat(format!($($arg)*)) };
}

#[macro_export]
macro_rules! internal_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::Internal(format!($($arg)*)) };
}

#[macro_export]
macro_rules! not_found_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::NotFound(format!($($arg)*)) };
}

#[macro_export]
macro_rules! already_exists_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::AlreadyExists(format!($($arg)*)) };
}

#[macro_export]
macro_rules! invalid_arg_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::InvalidArgument(format!($($arg)*)) };
}

#[macro_export]
macro_rules! not_supported_err {
    ($($arg:tt)*) => { $crate::error::ScaryError::NotSupported(format!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_display() {
        let err = ScaryError::Database("test error".to_string());
        assert_eq!(err.to_string(), "Database error: test error");
    }
    
    #[test]
    fn test_error_response() {
        let err = ScaryError::NotFound("database 'test' not found".to_string());
        let resp = ErrorResponse::from_error(&err);
        assert_eq!(resp.code, "NOT_FOUND");
        assert!(resp.details.is_some());
    }
}