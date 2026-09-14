//! File format management module for ScaryDB
//!
//! Provides unified import/export functionality for multiple file formats:
//! JSON, CSV, XML, YAML, TOML, BSON, CBOR, Parquet, Avro, XLSX

use arrow::array::{ArrayRef, RecordBatch};
use arrow::csv::ReaderBuilder as CsvReaderBuilder;
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::json::ReaderBuilder as JsonReaderBuilder;
// use parquet::arrow::ArrowWriter;
// use parquet::arrow::ParquetReader;
use parquet::file::properties::WriterProperties;
use bson::{Bson, Document};
use crate::{format_err, import_err, export_err, coll_err, doc_err, internal_err, ScaryError, Result, ScaryResultExt};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Supported file formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FileFormat {
    Json,
    JsonLines,
    Csv,
    Tsv,
    Xml,
    Yaml,
    Toml,
    Bson,
    Cbor,
    Parquet,
    Avro,
    Xlsx,
}

impl std::str::FromStr for FileFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(FileFormat::Json),
            "jsonl" | "ndjson" => Ok(FileFormat::JsonLines),
            "csv" => Ok(FileFormat::Csv),
            "tsv" => Ok(FileFormat::Tsv),
            "xml" => Ok(FileFormat::Xml),
            "yaml" | "yml" => Ok(FileFormat::Yaml),
            "toml" => Ok(FileFormat::Toml),
            "bson" => Ok(FileFormat::Bson),
            "cbor" => Ok(FileFormat::Cbor),
            "parquet" | "pq" => Ok(FileFormat::Parquet),
            "avro" => Ok(FileFormat::Avro),
            "xlsx" | "xls" => Ok(FileFormat::Xlsx),
            _ => Err(format!("Unknown file format: {}", s)),
        }
    }
}

impl FileFormat {
    /// Detect format from file extension
    pub fn from_extension(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "json" => Some(Self::Json),
            "jsonl" | "ndjson" => Some(Self::JsonLines),
            "csv" => Some(Self::Csv),
            "tsv" => Some(Self::Tsv),
            "xml" => Some(Self::Xml),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "bson" => Some(Self::Bson),
            "cbor" => Some(Self::Cbor),
            "parquet" | "pq" => Some(Self::Parquet),
            "avro" => Some(Self::Avro),
            "xlsx" | "xls" => Some(Self::Xlsx),
            _ => None,
        }
    }
    
    /// Get MIME type
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::JsonLines => "application/x-ndjson",
            Self::Csv => "text/csv",
            Self::Tsv => "text/tab-separated-values",
            Self::Xml => "application/xml",
            Self::Yaml => "application/yaml",
            Self::Toml => "application/toml",
            Self::Bson => "application/bson",
            Self::Cbor => "application/cbor",
            Self::Parquet => "application/parquet",
            Self::Avro => "application/avro",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        }
    }
    
    /// Get file extensions
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Json => &["json"],
            Self::JsonLines => &["jsonl", "ndjson"],
            Self::Csv => &["csv"],
            Self::Tsv => &["tsv"],
            Self::Xml => &["xml"],
            Self::Yaml => &["yaml", "yml"],
            Self::Toml => &["toml"],
            Self::Bson => &["bson"],
            Self::Cbor => &["cbor"],
            Self::Parquet => &["parquet", "pq"],
            Self::Avro => &["avro"],
            Self::Xlsx => &["xlsx", "xls"],
        }
    }
}

/// Import options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOptions {
    pub format: FileFormat,
    pub collection: String,
    pub database: Option<String>,
    pub batch_size: usize,
    pub skip_errors: bool,
    pub max_records: Option<usize>,
    pub fields: Option<Vec<String>>,
    pub transform: Option<TransformScript>,
    pub validation: ValidationLevel,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            format: FileFormat::Json,
            collection: String::new(),
            database: None,
            batch_size: 1000,
            skip_errors: false,
            max_records: None,
            fields: None,
            transform: None,
            validation: ValidationLevel::Strict,
        }
    }
}

/// Export options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub format: FileFormat,
    pub collection: String,
    pub database: Option<String>,
    pub query: Option<Document>,
    pub projection: Option<Document>,
    pub sort: Option<Document>,
    pub limit: Option<usize>,
    pub batch_size: usize,
    pub compression: CompressionType,
    pub fields: Option<Vec<String>>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: FileFormat::Json,
            collection: String::new(),
            database: None,
            query: None,
            projection: None,
            sort: None,
            limit: None,
            batch_size: 1000,
            compression: CompressionType::None,
            fields: None,
        }
    }
}

/// Transformation script for data processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformScript {
    pub script: String,
    pub language: TransformLanguage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransformLanguage {
    JavaScript,
    Rust,
    Wasm,
}

/// Validation level for imports
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationLevel {
    Strict,
    Lenient,
    None,
}

/// Compression type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    Gzip,
    Zstd,
    Snappy,
    Lz4,
}

/// File import statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportStats {
    pub total_records: usize,
    pub imported: usize,
    pub skipped: usize,
    pub errors: usize,
    pub duration_ms: u64,
    pub bytes_processed: u64,
}

/// File export statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportStats {
    pub total_records: usize,
    pub exported: usize,
    pub duration_ms: u64,
    pub bytes_written: u64,
    pub file_size: u64,
}

/// File format handler trait
pub trait FormatHandler: Send + Sync {
    fn format(&self) -> FileFormat;
    fn read(&self, path: &Path, options: &ImportOptions) -> Result<Vec<Document>>;
    fn write(&self, path: &Path, documents: &[Document], options: &ExportOptions) -> Result<ExportStats>;
    fn supports_streaming(&self) -> bool;
    fn streaming_read(&self, _path: &Path, _options: &ImportOptions) -> Result<Box<dyn DocumentStream>> {
        Err(ScaryError::NotSupported("Streaming not supported for this format".to_string()))
    }
}

/// Streaming document reader
pub trait DocumentStream: Send {
    fn next_batch(&mut self, size: usize) -> Result<Vec<Document>>;
    fn is_done(&self) -> bool;
}

/// JSON format handler
pub struct JsonHandler;

impl FormatHandler for JsonHandler {
    fn format(&self) -> FileFormat {
        FileFormat::Json
    }
    
    fn read(&self, path: &Path, options: &ImportOptions) -> Result<Vec<Document>> {
        let file = File::open(path).with_file_context(path.to_str().unwrap_or("unknown"))?;
        let reader = BufReader::new(file);
        
        // Try as array first
        let mut docs = Vec::new();
        let value: serde_json::Value = serde_json::from_reader(reader)
            .map_err(|e| format_err!("Failed to parse JSON: {}", e))?;
        
        match value {
            serde_json::Value::Array(arr) => {
                for (i, item) in arr.into_iter().enumerate() {
                    if options.max_records.map_or(false, |max| i >= max) {
                        break;
                    }
                    let doc = json_value_to_document(item)?;
                    docs.push(doc);
                }
            }
            serde_json::Value::Object(obj) => {
                let doc = json_value_to_document(serde_json::Value::Object(obj))?;
                docs.push(doc);
            }
            _ => return Err(format_err!("JSON root must be an object or array")),
        }
        
        Ok(docs)
    }
    
    fn write(&self, path: &Path, documents: &[Document], options: &ExportOptions) -> Result<ExportStats> {
        let file = File::create(path).with_file_context(path.to_str().unwrap_or("unknown"))?;
        let mut writer = BufWriter::new(file);
        
        let start = std::time::Instant::now();
        let mut bytes = 0;
        
        if documents.len() == 1 {
            let json = document_to_json(&documents[0])?;
            bytes += writer.write(json.as_bytes())?;
        } else {
            write!(writer, "[")?;
            bytes += 1;
            for (i, doc) in documents.iter().enumerate() {
                if i > 0 {
                    write!(writer, ",")?;
                    bytes += 1;
                }
                let json = document_to_json(doc)?;
                bytes += writer.write(json.as_bytes())?;
            }
            write!(writer, "]")?;
            bytes += 1;
        }
        
        writer.flush()?;
        
        Ok(ExportStats {
            total_records: documents.len(),
            exported: documents.len(),
            duration_ms: start.elapsed().as_millis() as u64,
            bytes_written: bytes as u64,
            file_size: bytes as u64,
        })
    }
    
    fn supports_streaming(&self) -> bool {
            true
        }
    }

    /// JSON Lines format handler
    pub struct JsonLinesHandler;

    impl FormatHandler for JsonLinesHandler {
        fn format(&self) -> FileFormat {
            FileFormat::JsonLines
        }
    
        fn read(&self, path: &Path, options: &ImportOptions) -> Result<Vec<Document>> {
            let file = File::open(path).with_file_context(path.to_str().unwrap_or("unknown"))?;
            let reader = BufReader::new(file);
            let mut docs = Vec::new();
        
            for (i, line) in std::io::BufRead::lines(reader).enumerate() {
                if options.max_records.map_or(false, |max| i >= max) {
                    break;
                }
                let line = line.map_err(|e| format_err!("Failed to read line {}: {}", i, e))?;
                if line.trim().is_empty() {
                    continue;
                }
                let value: serde_json::Value = serde_json::from_str(&line)
                    .map_err(|e| format_err!("Failed to parse JSON line {}: {}", i, e))?;
                let doc = json_value_to_document(value)?;
                docs.push(doc);
            }
        
            Ok(docs)
        }
    
        fn write(
            &self,
            path: &Path,
            documents: &[Document],
            options: &ExportOptions,
        ) -> Result<ExportStats> {
            let file = File::create(path).with_file_context(path.to_str().unwrap_or("unknown"))?;
            let mut writer = BufWriter::new(file);
            let start = std::time::Instant::now();
            let mut bytes = 0;
        
            for doc in documents {
                let json = document_to_json(doc)?;
                bytes += writer.write(json.as_bytes())?;
                bytes += writer.write(b"\n")?;
            }
        
            writer.flush()?;
        
            Ok(ExportStats {
                total_records: documents.len(),
                exported: documents.len(),
                duration_ms: start.elapsed().as_millis() as u64,
                bytes_written: bytes as u64,
                file_size: bytes as u64,
            })
        }
    
        fn supports_streaming(&self) -> bool {
            true
        }
    }

/// CSV format handler
pub struct CsvHandler {
    pub delimiter: u8,
}

impl CsvHandler {
    pub fn new(delimiter: u8) -> Self {
        Self { delimiter }
    }
}

impl FormatHandler for CsvHandler {
    fn format(&self) -> FileFormat {
        if self.delimiter == b'\t' {
            FileFormat::Tsv
        } else {
            FileFormat::Csv
        }
    }
    
    fn read(&self, path: &Path, options: &ImportOptions) -> Result<Vec<Document>> {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(true)
            .flexible(true)
            .from_path(path)
            .with_file_context(path.to_str().unwrap_or("unknown"))?;
        
        let headers = reader.headers()
            .map_err(|e| format_err!("Failed to read CSV headers: {}", e))?
            .clone();
        
        let mut docs = Vec::new();
        for (i, record) in reader.records().enumerate() {
            if options.max_records.map_or(false, |max| i >= max) {
                break;
            }
            let record = record.map_err(|e| format_err!("Failed to read CSV record {}: {}", i, e))?;
            
            let mut doc = Document::new();
            for (j, field) in record.iter().enumerate() {
                if j < headers.len() {
                    let value = csv_field_to_bson(field);
                    doc.insert(headers[j].to_string(), value);
                }
            }
            docs.push(doc);
        }
        
        Ok(docs)
    }
    
    fn write(&self, path: &Path, documents: &[Document], options: &ExportOptions) -> Result<ExportStats> {
        if documents.is_empty() {
            return Ok(ExportStats::default());
        }
        
        let file = File::create(path).with_file_context(path.to_str().unwrap_or("unknown"))?;
        let mut writer = csv::WriterBuilder::new()
            .delimiter(self.delimiter)
            .from_writer(BufWriter::new(file));
        
        // Get all unique fields
        let mut fields = std::collections::BTreeSet::new();
        for doc in documents {
            for key in doc.keys() {
                fields.insert(key.clone());
            }
        }
        let fields: Vec<String> = fields.into_iter().collect();
        
        // Write header
        writer.write_record(&fields)
            .map_err(|e| format_err!("Failed to write CSV header: {}", e))?;
        
        let start = std::time::Instant::now();
        let mut records_written = 0;
        
        for doc in documents {
            let mut record = Vec::with_capacity(fields.len());
            for field in &fields {
                let value = doc.get(field)
                    .map(bson_value_to_csv_string)
                    .unwrap_or_default();
                record.push(value);
            }
            writer.write_record(&record)
                .map_err(|e| format_err!("Failed to write CSV record: {}", e))?;
            records_written += 1;
        }
        
        writer.flush()
            .map_err(|e| format_err!("Failed to flush CSV writer: {}", e))?;
        
        let file_size = std::fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(0);
        
        Ok(ExportStats {
            total_records: documents.len(),
            exported: records_written,
            duration_ms: start.elapsed().as_millis() as u64,
            bytes_written: file_size,
            file_size,
        })
    }
    
    fn supports_streaming(&self) -> bool {
        true
    }
}

/// BSON format handler
pub struct BsonHandler;

impl FormatHandler for BsonHandler {
    fn format(&self) -> FileFormat {
        FileFormat::Bson
    }
    
    fn read(&self, path: &Path, options: &ImportOptions) -> Result<Vec<Document>> {
        let data = std::fs::read(path)
            .with_file_context(path.to_str().unwrap_or("unknown"))?;
        
        // Try as single document first
        if let Ok(doc) = bson::from_slice::<Document>(&data) {
            return Ok(vec![doc]);
        }
        
        // Try as array of documents
        if let Ok(docs) = bson::from_slice::<Vec<Document>>(&data) {
            return Ok(docs);
        }
        
        Err(format_err!("Invalid BSON format: expected document or array"))
    }
    
    fn write(&self, path: &Path, documents: &[Document], options: &ExportOptions) -> Result<ExportStats> {
        let data = if documents.len() == 1 {
            bson::to_vec(&documents[0])?
        } else {
            bson::to_vec(documents)?
        };
        
        let start = std::time::Instant::now();
        let file = File::create(path).with_file_context(path.to_str().unwrap_or("unknown"))?;
        let mut writer = BufWriter::new(file);
        let bytes = writer.write(&data)?;
        writer.flush()?;
        
        Ok(ExportStats {
            total_records: documents.len(),
            exported: documents.len(),
            duration_ms: start.elapsed().as_millis() as u64,
            bytes_written: bytes as u64,
            file_size: bytes as u64,
        })
    }
}

/// YAML format handler
pub struct YamlHandler;

impl FormatHandler for YamlHandler {
    fn format(&self) -> FileFormat {
        FileFormat::Yaml
    }
    
    fn read(&self, path: &Path, options: &ImportOptions) -> Result<Vec<Document>> {
        let content = std::fs::read_to_string(path)
            .with_file_context(path.to_str().unwrap_or("unknown"))?;
        
        let docs: Vec<serde_yaml::Value> = serde_yaml::Deserializer::from_str(&content)
            .map(|v| serde_yaml::Value::deserialize(v))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| format_err!("Failed to parse YAML: {}", e))?;
        
        let mut result = Vec::new();
        for (i, value) in docs.into_iter().enumerate() {
            if options.max_records.map_or(false, |max| i >= max) {
                break;
            }
            let doc = yaml_value_to_document(value)?;
            result.push(doc);
        }
        
        Ok(result)
    }
    
    fn write(&self, path: &Path, documents: &[Document], options: &ExportOptions) -> Result<ExportStats> {
        let start = std::time::Instant::now();
        let mut docs = Vec::new();
        
        for doc in documents {
            let yaml = document_to_yaml(doc)?;
            docs.push(yaml);
        }
        
        let content = if docs.len() == 1 {
            docs[0].clone()
        } else {
            let mut combined = String::new();
            for (i, doc) in docs.iter().enumerate() {
                if i > 0 {
                    combined.push_str("\n---\n");
                }
                combined.push_str(doc);
            }
            combined
        };
        
        let file = File::create(path).with_file_context(path.to_str().unwrap_or("unknown"))?;
        let mut writer = BufWriter::new(file);
        let bytes = writer.write(content.as_bytes())?;
        writer.flush()?;
        
        Ok(ExportStats {
            total_records: documents.len(),
            exported: documents.len(),
            duration_ms: start.elapsed().as_millis() as u64,
            bytes_written: bytes as u64,
            file_size: bytes as u64,
        })
    }
    
    fn supports_streaming(&self) -> bool {
        true
    }
}

/// Parquet format handler
pub struct ParquetHandler;

impl FormatHandler for ParquetHandler {
    fn format(&self) -> FileFormat {
        FileFormat::Parquet
    }
    
    fn read(&self, path: &Path, options: &ImportOptions) -> Result<Vec<Document>> {
        // Parquet support disabled for now - requires parquet crate
        let _ = (path, options);
        Ok(vec![])
    }
    
    fn write(&self, path: &Path, documents: &[Document], options: &ExportOptions) -> Result<ExportStats> {
        // Parquet support disabled for now
        let _ = (path, documents, options);
        Ok(ExportStats::default())
    }
    
    fn supports_streaming(&self) -> bool {
        false
    }
}

/// File manager for import/export operations
pub struct FileManager {
    handlers: HashMap<FileFormat, Box<dyn FormatHandler>>,
}

impl FileManager {
    pub fn new() -> Self {
        let mut handlers: HashMap<FileFormat, Box<dyn FormatHandler>> = HashMap::new();
        handlers.insert(FileFormat::Json, Box::new(JsonHandler));
        handlers.insert(FileFormat::JsonLines, Box::new(JsonLinesHandler));
        handlers.insert(FileFormat::Csv, Box::new(CsvHandler::new(b',')));
        handlers.insert(FileFormat::Tsv, Box::new(CsvHandler::new(b'\t')));
        handlers.insert(FileFormat::Bson, Box::new(BsonHandler));
        handlers.insert(FileFormat::Yaml, Box::new(YamlHandler));
        handlers.insert(FileFormat::Parquet, Box::new(ParquetHandler));
        // TODO: Add XML, TOML, CBOR, Avro, XLSX handlers
        
        Self { handlers }
    }
    
    pub fn register_handler(&mut self, handler: Box<dyn FormatHandler>) {
        self.handlers.insert(handler.format(), handler);
    }
    
    pub fn import(&self, path: &Path, options: &ImportOptions) -> Result<ImportStats> {
        let format = options.format;
        let handler = self.handlers.get(&format)
            .ok_or_else(|| format_err!("No handler for format: {:?}", format))?;
        
        let start = std::time::Instant::now();
        let documents = handler.read(path, options)?;
        let duration = start.elapsed();
        
        Ok(ImportStats {
            total_records: documents.len(),
            imported: documents.len(),
            skipped: 0,
            errors: 0,
            duration_ms: duration.as_millis() as u64,
            bytes_processed: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        })
    }
    
    pub fn export(&self, path: &Path, documents: &[Document], options: &ExportOptions) -> Result<ExportStats> {
        let format = options.format;
        let handler = self.handlers.get(&format)
            .ok_or_else(|| format_err!("No handler for format: {:?}", format))?;
        
        handler.write(path, documents, options)
    }
    
    pub fn detect_format(&self, path: &Path) -> Option<FileFormat> {
        FileFormat::from_extension(path)
    }
    
    pub fn list_formats(&self) -> Vec<FileFormat> {
        self.handlers.keys().copied().collect()
    }
}

/// Helper functions for format conversion

fn json_value_to_document(value: serde_json::Value) -> Result<Document> {
    let mut doc = Document::new();
    if let serde_json::Value::Object(map) = value {
        for (key, val) in map {
            doc.insert(key, json_value_to_bson(val));
        }
    } else {
        return Err(format_err!("JSON value must be an object"));
    }
    Ok(doc)
}

fn json_value_to_bson(value: serde_json::Value) -> Bson {
    match value {
        serde_json::Value::Null => Bson::Null,
        serde_json::Value::Bool(b) => Bson::Boolean(b),
        serde_json::Value::Number(n) => {
            if n.is_i64() {
                Bson::Int64(n.as_i64().unwrap())
            } else if n.is_u64() {
                Bson::Int64(n.as_u64().unwrap() as i64)
            } else {
                Bson::Double(n.as_f64().unwrap())
            }
        }
        serde_json::Value::String(s) => Bson::String(s),
        serde_json::Value::Array(arr) => Bson::Array(arr.into_iter().map(json_value_to_bson).collect()),
        serde_json::Value::Object(obj) => {
            let mut doc = Document::new();
            for (k, v) in obj {
                doc.insert(k, json_value_to_bson(v));
            }
            Bson::Document(doc)
        }
    }
}

fn document_to_json(doc: &Document) -> Result<String> {
    let mut json = serde_json::Map::new();
    for (key, val) in doc {
        json.insert(key.clone(), bson_value_to_json(val.clone())?);
    }
    Ok(serde_json::to_string(&json)?)
}

fn bson_value_to_json(value: Bson) -> Result<serde_json::Value> {
    match value {
        Bson::Null => Ok(serde_json::Value::Null),
        Bson::Boolean(b) => Ok(serde_json::Value::Bool(b)),
        Bson::Int32(i) => Ok(serde_json::Value::Number(i.into())),
        Bson::Int64(i) => Ok(serde_json::Value::Number(i.into())),
        Bson::Double(f) => Ok(serde_json::Value::Number(
            serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0))
        )),
        Bson::String(s) => Ok(serde_json::Value::String(s)),
        Bson::Array(arr) => {
            let mut vec = Vec::new();
            for item in arr {
                vec.push(bson_value_to_json(item)?);
            }
            Ok(serde_json::Value::Array(vec))
        }
        Bson::Document(doc) => {
            let mut map = serde_json::Map::new();
            for (k, v) in doc {
                map.insert(k, bson_value_to_json(v)?);
            }
            Ok(serde_json::Value::Object(map))
        }
        Bson::Binary(b) => Ok(serde_json::Value::String(base64::encode(&b.bytes))),
        Bson::ObjectId(id) => Ok(serde_json::Value::String(id.to_hex())),
        Bson::DateTime(dt) => Ok(serde_json::Value::String(dt.try_to_rfc3339_string().unwrap_or_default())),
        _ => Ok(serde_json::Value::String(format!("{:?}", value))),
    }
}

fn csv_field_to_bson(field: &str) -> Bson {
    // Try to parse as number
    if let Ok(i) = field.parse::<i64>() {
        return Bson::Int64(i);
    }
    if let Ok(f) = field.parse::<f64>() {
        return Bson::Double(f);
    }
    if field.eq_ignore_ascii_case("true") {
        return Bson::Boolean(true);
    }
    if field.eq_ignore_ascii_case("false") {
        return Bson::Boolean(false);
    }
    if field.is_empty() || field.eq_ignore_ascii_case("null") {
        return Bson::Null;
    }
    Bson::String(field.to_string())
}

fn bson_value_to_csv_string(value: &Bson) -> String {
    match value {
        Bson::Null => String::new(),
        Bson::Boolean(b) => b.to_string(),
        Bson::Int32(i) => i.to_string(),
        Bson::Int64(i) => i.to_string(),
        Bson::Double(f) => f.to_string(),
        Bson::String(s) => s.clone(),
        Bson::Binary(b) => base64::encode(&b.bytes),
        Bson::ObjectId(id) => id.to_hex(),
        Bson::DateTime(dt) => dt.try_to_rfc3339_string().unwrap_or_default(),
        _ => format!("{:?}", value),
    }
}

fn yaml_value_to_document(value: serde_yaml::Value) -> Result<Document> {
    let mut doc = Document::new();
    if let serde_yaml::Value::Mapping(map) = value {
        for (key, val) in map {
            if let Some(key_str) = key.as_str() {
                doc.insert(key_str.to_string(), yaml_value_to_bson(val));
            }
        }
    } else {
        return Err(format_err!("YAML value must be a mapping"));
    }
    Ok(doc)
}

fn yaml_value_to_bson(value: serde_yaml::Value) -> Bson {
    match value {
        serde_yaml::Value::Null => Bson::Null,
        serde_yaml::Value::Bool(b) => Bson::Boolean(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Bson::Int64(i)
            } else if let Some(u) = n.as_u64() {
                Bson::Int64(u as i64)
            } else if let Some(f) = n.as_f64() {
                Bson::Double(f)
            } else {
                Bson::Null
            }
        }
        serde_yaml::Value::String(s) => Bson::String(s),
        serde_yaml::Value::Sequence(seq) => Bson::Array(seq.into_iter().map(yaml_value_to_bson).collect()),
        serde_yaml::Value::Mapping(map) => {
            let mut doc = Document::new();
            for (k, v) in map {
                if let Some(key_str) = k.as_str() {
                    doc.insert(key_str.to_string(), yaml_value_to_bson(v));
                }
            }
            Bson::Document(doc)
        }
        _ => Bson::Null,
    }
}

fn document_to_yaml(doc: &Document) -> Result<String> {
    let mut yaml = serde_yaml::Mapping::new();
    for (key, val) in doc {
        yaml.insert(serde_yaml::Value::String(key.clone()), bson_value_to_yaml(val.clone()));
    }
    Ok(serde_yaml::to_string(&yaml)?)
}

fn bson_value_to_yaml(value: Bson) -> serde_yaml::Value {
    match value {
        Bson::Null => serde_yaml::Value::Null,
        Bson::Boolean(b) => serde_yaml::Value::Bool(b),
        Bson::Int32(i) => serde_yaml::Value::Number(i.into()),
        Bson::Int64(i) => serde_yaml::Value::Number(i.into()),
        Bson::Double(f) => serde_yaml::Value::Number(serde_yaml::Number::from(f)),
        Bson::String(s) => serde_yaml::Value::String(s),
        Bson::Array(arr) => serde_yaml::Value::Sequence(arr.into_iter().map(bson_value_to_yaml).collect()),
        Bson::Document(doc) => {
            let mut map = serde_yaml::Mapping::new();
            for (k, v) in doc {
                map.insert(serde_yaml::Value::String(k), bson_value_to_yaml(v));
            }
            serde_yaml::Value::Mapping(map)
        }
        _ => serde_yaml::Value::String(format!("{:?}", value)),
    }
}

fn documents_to_arrow_schema(documents: &[Document]) -> Result<SchemaRef> {
    if documents.is_empty() {
        return Ok(Arc::new(Schema::empty()));
    }
    
    let mut fields = Vec::new();
    let mut field_types: HashMap<String, DataType> = HashMap::new();
    
    // Infer types from first document
    for (key, val) in &documents[0] {
        let dt = bson_to_arrow_type(val);
        field_types.insert(key.clone(), dt);
    }
    
    for (key, dt) in field_types {
        fields.push(Field::new(key, dt, true));
    }
    
    Ok(Arc::new(Schema::new(fields)))
}

fn bson_to_arrow_type(value: &Bson) -> DataType {
    match value {
        Bson::Null => DataType::Null,
        Bson::Boolean(_) => DataType::Boolean,
        Bson::Int32(_) => DataType::Int32,
        Bson::Int64(_) => DataType::Int64,
        Bson::Double(_) => DataType::Float64,
        Bson::String(_) => DataType::Utf8,
        Bson::Binary(_) => DataType::Binary,
        Bson::ObjectId(_) => DataType::Utf8,
        Bson::DateTime(_) => DataType::Timestamp(arrow::datatypes::TimeUnit::Millisecond, None),
        Bson::Array(_) => DataType::List(Arc::new(Field::new("item", DataType::Null, true))),
        Bson::Document(_) => DataType::Struct(arrow::datatypes::Fields::empty()),
        _ => DataType::Utf8,
    }
}

fn documents_to_record_batch(documents: &[Document], schema: &SchemaRef) -> Result<RecordBatch> {
    // This is a simplified implementation
    // In production, would use proper Arrow array builders
    let mut columns: Vec<ArrayRef> = Vec::new();
    
    for field in schema.fields() {
        let name = field.name();
        let data_type = field.data_type();
        
        // Build array based on type
        let array = match data_type {
            DataType::Int64 => {
                let mut builder = arrow::array::Int64Builder::new();
                for doc in documents {
                    if let Some(Bson::Int64(v)) = doc.get(name) {
                        builder.append_value(*v);
                    } else if let Some(Bson::Int32(v)) = doc.get(name) {
                        builder.append_value(*v as i64);
                    } else {
                        builder.append_null();
                    }
                }
                Arc::new(builder.finish())
            }
            DataType::Float64 => {
                let mut builder = arrow::array::Float64Builder::new();
                for doc in documents {
                    if let Some(Bson::Double(v)) = doc.get(name) {
                        builder.append_value(*v);
                    } else if let Some(Bson::Int64(v)) = doc.get(name) {
                        builder.append_value(*v as f64);
                    } else {
                        builder.append_null();
                    }
                }
                Arc::new(builder.finish())
            }
            DataType::Utf8 => {
                let mut builder = arrow::array::StringBuilder::new();
                for doc in documents {
                    if let Some(v) = doc.get(name) {
                        builder.append_value(bson_value_to_csv_string(v));
                    } else {
                        builder.append_null();
                    }
                }
                Arc::new(builder.finish())
            }
            _ => {
                let mut builder = arrow::array::StringBuilder::new();
                for doc in documents {
                    if let Some(v) = doc.get(name) {
                        builder.append_value(bson_value_to_csv_string(v));
                    } else {
                        builder.append_null();
                    }
                }
                Arc::new(builder.finish())
            }
        };
        columns.push(array);
    }
    
    RecordBatch::try_new(schema.clone(), columns)
        .map_err(|e| format_err!("Failed to create record batch: {}", e))
}

fn record_batch_to_documents(batch: &RecordBatch, schema: &SchemaRef) -> Result<Vec<Document>> {
    let mut docs = Vec::new();
    let num_rows = batch.num_rows();
    
    for row_idx in 0..num_rows {
        let mut doc = Document::new();
        for (col_idx, field) in schema.fields().iter().enumerate() {
            let column = batch.column(col_idx);
            let value = array_value_to_bson(column, row_idx)?;
            doc.insert(field.name().clone(), value);
        }
        docs.push(doc);
    }
    
    Ok(docs)
}

fn array_value_to_bson(array: &ArrayRef, row_idx: usize) -> Result<Bson> {
    use arrow::array::*;
    
    let data_type = array.data_type();
    match data_type {
        DataType::Int64 => {
            let arr = array.as_any().downcast_ref::<Int64Array>().unwrap();
            if arr.is_null(row_idx) {
                Ok(Bson::Null)
            } else {
                Ok(Bson::Int64(arr.value(row_idx)))
            }
        }
        DataType::Float64 => {
            let arr = array.as_any().downcast_ref::<Float64Array>().unwrap();
            if arr.is_null(row_idx) {
                Ok(Bson::Null)
            } else {
                Ok(Bson::Double(arr.value(row_idx)))
            }
        }
        DataType::Utf8 => {
            let arr = array.as_any().downcast_ref::<StringArray>().unwrap();
            if arr.is_null(row_idx) {
                Ok(Bson::Null)
            } else {
                Ok(Bson::String(arr.value(row_idx).to_string()))
            }
        }
        DataType::Boolean => {
            let arr = array.as_any().downcast_ref::<BooleanArray>().unwrap();
            if arr.is_null(row_idx) {
                Ok(Bson::Null)
            } else {
                Ok(Bson::Boolean(arr.value(row_idx)))
            }
        }
        _ => Ok(Bson::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_json_handler() {
        let handler = JsonHandler;
        let doc = doc! {"name": "test", "value": 42};
        
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();
        
        let options = ExportOptions {
            format: FileFormat::Json,
            ..Default::default()
        };
        
        let stats = handler.write(path, &[doc.clone()], &options).unwrap();
        assert_eq!(stats.exported, 1);
        
        let import_options = ImportOptions {
            format: FileFormat::Json,
            ..Default::default()
        };
        
        let docs = handler.read(path, &import_options).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].get_str("name").unwrap(), "test");
    }
    
    #[test]
    fn test_csv_handler() {
        let handler = CsvHandler::new(b',');
        let doc = doc! {"name": "test", "age": 30, "active": true};
        
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();
        
        let options = ExportOptions {
            format: FileFormat::Csv,
            ..Default::default()
        };
        
        let stats = handler.write(path, &[doc.clone()], &options).unwrap();
        assert_eq!(stats.exported, 1);
        
        let import_options = ImportOptions {
            format: FileFormat::Csv,
            ..Default::default()
        };
        
        let docs = handler.read(path, &import_options).unwrap();
        assert_eq!(docs.len(), 1);
    }
    
    #[test]
    fn test_format_detection() {
        assert_eq!(FileFormat::from_extension(Path::new("test.json")), Some(FileFormat::Json));
        assert_eq!(FileFormat::from_extension(Path::new("test.csv")), Some(FileFormat::Csv));
        assert_eq!(FileFormat::from_extension(Path::new("test.bson")), Some(FileFormat::Bson));
        assert_eq!(FileFormat::from_extension(Path::new("test.parquet")), Some(FileFormat::Parquet));
    }
}