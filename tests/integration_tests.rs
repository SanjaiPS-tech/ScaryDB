//! Comprehensive integration tests for ScaryDB

use scarydb::{
    document::{Value, DocumentRecord, QueryFilter, UpdateSpec, PipelineStage, IndexDef, CollectionOptions, ValidationLevel, ValidationAction},
    error::Result,
    file_manager::{FileManager, FileFormat, ImportOptions, ExportOptions},
    parser::{Parser, Command},
    storage::{StorageEngine, StorageConfig},
    worker::{WorkerPool, Response, CommandStats},
};
use bson::{doc, Document, Bson};
use std::sync::Arc;
use tempfile::NamedTempFile;

fn create_test_engine() -> Arc<StorageEngine> {
    let config = StorageConfig::default();
    Arc::new(StorageEngine::new(config).unwrap())
}

fn create_test_worker_pool() -> Arc<WorkerPool> {
    let storage = create_test_engine();
    let file_manager = Arc::new(FileManager::new());
    Arc::new(WorkerPool::new(2, storage, file_manager))
}

#[test]
fn test_document_creation_and_conversion() {
    // Test basic value creation
    let v = Value::string("test");
    assert_eq!(v.type_name(), "string");
    assert!(v.is_string());
    assert_eq!(v.as_str(), Some("test"));
    
    let v = Value::int64(42);
    assert_eq!(v.type_name(), "int64");
    assert!(v.is_number());
    assert_eq!(v.as_i64(), Some(42));
    
    let v = Value::double(3.14);
    assert_eq!(v.type_name(), "double");
    assert!(v.is_number());
    assert_eq!(v.as_f64(), Some(3.14));
    
    let v = Value::bool(true);
    assert_eq!(v.type_name(), "bool");
    assert!(v.as_bool() == Some(true));
    
    // Test array
    let v = Value::array(vec![Value::int32(1), Value::int32(2), Value::int32(3)]);
    assert_eq!(v.type_name(), "array");
    assert!(v.is_array());
    assert_eq!(v.array_len(), Some(3));
    
    // Test object
    let doc = doc! { "name": "John", "age": 30 };
    let v = Value::object(doc.clone());
    assert_eq!(v.type_name(), "object");
    assert!(v.is_object());
    assert_eq!(v.object_keys(), Some(vec!["age".to_string(), "name".to_string()]));
}

#[test]
fn test_bson_conversion_roundtrip() {
    let v = Value::string("hello");
    let bson = v.to_bson();
    let v2 = Value::from_bson(bson);
    assert_eq!(v, v2);
    
    let v = Value::int64(42);
    let bson = v.to_bson();
    let v2 = Value::from_bson(bson);
    assert_eq!(v, v2);
    
    let v = Value::object(doc! {"a": 1, "b": "test", "c": [1, 2, 3]});
    let bson = v.to_bson();
    let v2 = Value::from_bson(bson);
    assert_eq!(v, v2);
    
    let v = Value::array(vec![Value::int32(1), Value::string("test"), Value::bool(false)]);
    let bson = v.to_bson();
    let v2 = Value::from_bson(bson);
    assert_eq!(v, v2);
}

#[test]
fn test_json_conversion() {
    let v = Value::string("hello");
    let json = v.to_json();
    let v2 = Value::from_json(json).unwrap();
    assert_eq!(v, v2);
    
    let v = Value::object(doc! {"name": "John", "age": 30, "tags": ["admin", "user"]});
    let json = v.to_json();
    let v2 = Value::from_json(json).unwrap();
    assert_eq!(v, v2);
}

#[test]
fn test_query_filter_creation() {
    // Test equality filter
    let filter = QueryFilter::eq("name", Value::string("John"));
    let doc = doc! { "name": "John", "age": 30 };
    assert!(filter.matches(&doc));
    
    // Test comparison filters
    let filter = QueryFilter::gt("age", Value::int32(25));
    let doc = doc! { "age": 30 };
    assert!(filter.matches(&doc));
    
    let filter = QueryFilter::lt("age", Value::int32(35));
    let doc = doc! { "age": 30 };
    assert!(filter.matches(&doc));
    
    // Test in operator
    let filter = QueryFilter::in_array("status", vec![Value::string("active"), Value::string("pending")]);
    let doc = doc! { "status": "active" };
    assert!(filter.matches(&doc));
    
    // Test logical operators
    let filter = QueryFilter::and(vec![
        QueryFilter::eq("name", Value::string("John")),
        QueryFilter::gt("age", Value::int32(25)),
    ]);
    let doc = doc! { "name": "John", "age": 30 };
    assert!(filter.matches(&doc));
}

#[test]
fn test_update_spec() {
    let update = UpdateSpec::new()
        .set("name", Value::string("Jane"))
        .inc("age", 1)
        .mul("score", 1.5)
        .set_on_insert("created_at", Value::datetime_chrono(chrono::Utc::now()));
    
    let bson = update.operations;
    assert!(bson.contains_key("$set"));
    assert!(bson.contains_key("$inc"));
    assert!(bson.contains_key("$mul"));
    assert!(bson.contains_key("$setOnInsert"));
}

#[test]
fn test_parser_simple_commands() {
    let mut parser = Parser::new("USE mydb;");
    let cmd = parser.parse_command().unwrap();
    assert!(matches!(cmd, Command::UseDatabase { name } if name == "mydb"));
    
    let mut parser = Parser::new("CREATE COLLECTION users { capped: true, size: 1000 };");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::CreateCollection { name, options } => {
            assert_eq!(name, "users");
            assert!(options.capped);
            assert_eq!(options.size, Some(1000));
        }
        _ => panic!("Expected CreateCollection"),
    }
}

#[test]
fn test_parser_find_commands() {
    let mut parser = Parser::new("FIND users WHERE name = 'John' AND age > 25;");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Find { collection, filter, .. } => {
            assert_eq!(collection, "users");
            assert!(filter.is_some());
        }
        _ => panic!("Expected Find"),
    }
    
    let mut parser = Parser::new("SELECT name, age FROM users WHERE age > 18 ORDER BY age DESC LIMIT 10;");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Find { collection, projection, sort, limit, .. } => {
            assert_eq!(collection, "users");
            assert!(projection.is_some());
            assert!(sort.is_some());
            assert_eq!(limit, Some(10));
        }
        _ => panic!("Expected Find"),
    }
}

#[test]
fn test_parser_insert_commands() {
    let mut parser = Parser::new("INSERT INTO users { name: 'John', age: 30 };");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Insert { collection, documents, .. } => {
            assert_eq!(collection, "users");
            assert_eq!(documents.len(), 1);
        }
        _ => panic!("Expected Insert"),
    }
    
    let mut parser = Parser::new("INSERT INTO users [ { name: 'John' }, { name: 'Jane' } ];");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Insert { collection, documents, .. } => {
            assert_eq!(collection, "users");
            assert_eq!(documents.len(), 2);
        }
        _ => panic!("Expected Insert"),
    }
}

#[test]
fn test_parser_update_delete() {
    let mut parser = Parser::new("UPDATE users SET name = 'Jane', age += 1 WHERE name = 'John';");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Update { collection, filter, update, .. } => {
            assert_eq!(collection, "users");
            assert!(filter.is_some());
            assert!(update.operations.contains_key("$set") || update.operations.contains_key("$inc"));
        }
        _ => panic!("Expected Update"),
    }
    
    let mut parser = Parser::new("DELETE FROM users WHERE status = 'inactive' LIMIT 5;");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Delete { collection, filter, limit } => {
            assert_eq!(collection, "users");
            assert!(filter.is_some());
            assert_eq!(limit, Some(5));
        }
        _ => panic!("Expected Delete"),
    }
}

#[test]
fn test_parser_aggregate() {
    let mut parser = Parser::new("AGGREGATE users [ { $match: { age: { $gt: 25 } } }, { $group: { _id: '$city', count: { $sum: 1 } } } ];");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Aggregate { collection, pipeline } => {
            assert_eq!(collection, "users");
            assert_eq!(pipeline.len(), 2);
            assert_eq!(pipeline[0].stage, "$match");
            assert_eq!(pipeline[1].stage, "$group");
        }
        _ => panic!("Expected Aggregate"),
    }
}

#[test]
fn test_parser_index_commands() {
    let mut parser = Parser::new("INDEX CREATE idx_name ON users { name: 1, age: -1 } WITH unique, sparse;");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::CreateIndex { collection, index } => {
            assert_eq!(collection, "users");
            assert_eq!(index.name, "idx_name");
            assert!(index.unique);
            assert!(index.sparse);
        }
        _ => panic!("Expected CreateIndex"),
    }
}

#[test]
fn test_parser_transaction() {
    let mut parser = Parser::new("BEGIN TRANSACTION;");
    let cmd = parser.parse_command().unwrap();
    assert!(matches!(cmd, Command::BeginTransaction { .. }));
    
    let mut parser = Parser::new("COMMIT TRANSACTION;");
    let cmd = parser.parse_command().unwrap();
    assert!(matches!(cmd, Command::CommitTransaction));
    
    let mut parser = Parser::new("ROLLBACK TRANSACTION;");
    let cmd = parser.parse_command().unwrap();
    assert!(matches!(cmd, Command::RollbackTransaction));
}

#[test]
fn test_file_manager_json() {
    let file_manager = FileManager::new();
    let doc = doc! { "name": "test", "value": 42, "tags": ["a", "b"] };
    
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();
    
    let export_options = ExportOptions {
        format: FileFormat::Json,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    let stats = file_manager.export(path, &[doc.clone()], &export_options).unwrap();
    assert_eq!(stats.exported, 1);
    
    let import_options = ImportOptions {
        format: FileFormat::Json,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    let stats = file_manager.import(path, &import_options).unwrap();
    assert_eq!(stats.imported, 1);
}

#[test]
fn test_file_manager_csv() {
    let file_manager = FileManager::new();
    let doc1 = doc! { "name": "John", "age": 30, "city": "NYC" };
    let doc2 = doc! { "name": "Jane", "age": 25, "city": "LA" };
    
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();
    
    let export_options = ExportOptions {
        format: FileFormat::Csv,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    let stats = file_manager.export(path, &[doc1, doc2], &export_options).unwrap();
    assert_eq!(stats.exported, 2);
    
    let import_options = ImportOptions {
        format: FileFormat::Csv,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    let stats = file_manager.import(path, &import_options).unwrap();
    assert_eq!(stats.imported, 2);
}

#[test]
fn test_file_manager_bson() {
    let file_manager = FileManager::new();
    let doc = doc! { "name": "test", "value": 42 };
    
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();
    
    let export_options = ExportOptions {
        format: FileFormat::Bson,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    let stats = file_manager.export(path, &[doc.clone()], &export_options).unwrap();
    assert_eq!(stats.exported, 1);
    
    let import_options = ImportOptions {
        format: FileFormat::Bson,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    let stats = file_manager.import(path, &import_options).unwrap();
    assert_eq!(stats.imported, 1);
}

#[test]
fn test_file_manager_yaml() {
    let file_manager = FileManager::new();
    let doc = doc! { "name": "test", "value": 42, "nested": { "a": 1 } };
    
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();
    
    let export_options = ExportOptions {
        format: FileFormat::Yaml,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    let stats = file_manager.export(path, &[doc.clone()], &export_options).unwrap();
    assert_eq!(stats.exported, 1);
    
    let import_options = ImportOptions {
        format: FileFormat::Yaml,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    let stats = file_manager.import(path, &import_options).unwrap();
    assert_eq!(stats.imported, 1);
}

#[test]
fn test_storage_engine_basic() {
    let config = StorageConfig::default();
    let engine = StorageEngine::new(config).unwrap();
    
    // Test database creation
    engine.create_database("testdb".to_string(), None).unwrap();
    assert!(engine.get_database("testdb").is_some());
    
    // Test listing databases
    let dbs = engine.list_databases();
    assert!(dbs.contains(&"testdb".to_string()));
    
    // Test dropping database
    engine.drop_database("testdb").unwrap();
    assert!(engine.get_database("testdb").is_none());
}

#[test]
fn test_storage_engine_global_stats() {
    let config = StorageConfig::default();
    let engine = StorageEngine::new(config).unwrap();
    
    engine.create_database("db1".to_string(), None).unwrap();
    engine.create_database("db2".to_string(), None).unwrap();
    
    let stats = engine.get_global_stats();
    assert_eq!(stats.total_databases, 2);
}

#[test]
fn test_worker_pool_execution() {
    let pool = create_test_worker_pool();
    
    // Test simple command
    let response = pool.execute(Command::UseDatabase { name: "test".to_string() });
    // Will fail because DB doesn't exist, but should not panic
    assert!(response.is_err() || !response.unwrap().success);
    
    pool.shutdown();
}

#[test]
fn test_response_helpers() {
    let resp = Response::ok("test");
    assert!(resp.success);
    
    let resp = Response::error("error");
    assert!(!resp.success);
    assert_eq!(resp.error, Some("error".to_string()));
    
    let doc = doc! { "test": "data" };
    let resp = Response::data(doc.clone());
    assert!(resp.success);
    assert_eq!(resp.data, Some(doc));
    
    let docs = vec![doc! { "a": 1 }, doc! { "b": 2 }];
    let resp = Response::documents(docs.clone());
    assert!(resp.success);
    assert_eq!(resp.documents, Some(docs));
}

#[test]
fn test_storage_engine_shutdown() {
    let config = StorageConfig::default();
    let engine = StorageEngine::new(config).unwrap();
    
    engine.shutdown().unwrap();
}

#[test]
fn test_worker_pool_shutdown() {
    let pool = create_test_worker_pool();
    pool.shutdown();
}

#[test]
fn test_multiple_commands_parsing() {
    let input = r#"
        USE testdb;
        CREATE COLLECTION users;
        INSERT INTO users { name: 'John', age: 30 };
        FIND users WHERE age > 25;
    "#;
    
    let mut parser = Parser::new(input);
    let commands = parser.parse_commands().unwrap();
    assert_eq!(commands.len(), 4);
}

#[test]
fn test_complex_filter_parsing() {
    let mut parser = Parser::new("FIND users WHERE (age > 18 AND age < 65) OR status = 'admin';");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Find { filter, .. } => {
            assert!(filter.is_some());
        }
        _ => panic!("Expected Find"),
    }
}

#[test]
fn test_gridfs_commands_parsing() {
    let mut parser = Parser::new("GRIDFS UPLOAD test.txt FROM /path/to/file CONTENT_TYPE text/plain;");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::GridFSUpload { filename, path, content_type } => {
            assert_eq!(filename, "test.txt");
            assert_eq!(path, "/path/to/file");
            assert_eq!(content_type, Some("text/plain".to_string()));
        }
        _ => panic!("Expected GridFSUpload"),
    }
}

#[test]
fn test_import_export_commands_parsing() {
    let mut parser = Parser::new("IMPORT JSON FROM /data/users.json INTO users OPTIONS { batchSize: 1000 };");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Import { format, path, collection, options } => {
            assert_eq!(format, "JSON");
            assert_eq!(path, "/data/users.json");
            assert_eq!(collection, "users");
            assert!(options.is_some());
        }
        _ => panic!("Expected Import"),
    }
    
    let mut parser = Parser::new("EXPORT CSV FROM users TO /data/users.csv;");
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::Export { format, collection, path, .. } => {
            assert_eq!(format, "CSV");
            assert_eq!(collection, "users");
            assert_eq!(path, "/data/users.csv");
        }
        _ => panic!("Expected Export"),
    }
}

#[test]
fn test_collection_options_parsing() {
    let mut parser = Parser::new(r#"CREATE COLLECTION users { capped: true, size: 10000, max: 1000, validationLevel: "strict", validationAction: "error" };"#);
    let cmd = parser.parse_command().unwrap();
    match cmd {
        Command::CreateCollection { name, options } => {
            assert_eq!(name, "users");
            assert!(options.capped);
            assert_eq!(options.size, Some(10000));
            assert_eq!(options.max, Some(1000));
            assert_eq!(options.validation_level, ValidationLevel::Strict);
            assert_eq!(options.validation_action, ValidationAction::Error);
        }
        _ => panic!("Expected CreateCollection"),
    }
}

#[test]
fn test_document_record() {
    let doc = doc! { "name": "John", "age": 30 };
    let record = DocumentRecord::new(doc.clone());
    
    assert!(!record._id.to_string().is_empty());
    assert_eq!(record.data.get_str("name").unwrap(), "John");
    assert_eq!(record.version, 1);
    
    let full_doc = record.to_document();
    assert!(full_doc.contains_key("_id"));
    assert!(full_doc.contains_key("_created_at"));
    assert!(full_doc.contains_key("_updated_at"));
    assert!(full_doc.contains_key("_version"));
}

#[test]
fn test_index_def_creation() {
    let index = IndexDef::new("idx_name".to_string(), doc! { "name": 1, "age": -1 })
        .unique()
        .sparse()
        .ttl(3600);
    
    assert_eq!(index.name, "idx_name");
    assert!(index.unique);
    assert!(index.sparse);
    assert_eq!(index.expire_after_seconds, Some(3600));
}

#[test]
fn test_collection_options_default() {
    let options = CollectionOptions::default();
    assert!(!options.capped);
    assert_eq!(options.size, None);
    assert_eq!(options.max, None);
    assert_eq!(options.validation_level, ValidationLevel::Strict);
    assert_eq!(options.validation_action, ValidationAction::Error);
}

#[test]
fn test_pipeline_stage_creation() {
    let stage = PipelineStage::match_stage(doc! { "age": { "$gt": 25 } });
    assert_eq!(stage.stage, "$match");
    
    let stage = PipelineStage::group_stage(doc! { "_id": "$city", "count": { "$sum": 1 } });
    assert_eq!(stage.stage, "$group");
    
    let stage = PipelineStage::lookup_stage("orders", "user_id", "_id", "orders");
    assert_eq!(stage.stage, "$lookup");
}

#[test]
fn test_file_format_detection() {
    assert_eq!(FileFormat::from_extension(std::path::Path::new("test.json")), Some(FileFormat::Json));
    assert_eq!(FileFormat::from_extension(std::path::Path::new("test.jsonl")), Some(FileFormat::JsonLines));
    assert_eq!(FileFormat::from_extension(std::path::Path::new("test.csv")), Some(FileFormat::Csv));
    assert_eq!(FileFormat::from_extension(std::path::Path::new("test.bson")), Some(FileFormat::Bson));
    assert_eq!(FileFormat::from_extension(std::path::Path::new("test.parquet")), Some(FileFormat::Parquet));
    assert_eq!(FileFormat::from_extension(std::path::Path::new("test.yaml")), Some(FileFormat::Yaml));
    assert_eq!(FileFormat::from_extension(std::path::Path::new("test.yml")), Some(FileFormat::Yaml));
    assert_eq!(FileFormat::from_extension(std::path::Path::new("unknown.xyz")), None);
}

#[test]
fn test_file_format_mime_types() {
    assert_eq!(FileFormat::Json.mime_type(), "application/json");
    assert_eq!(FileFormat::Csv.mime_type(), "text/csv");
    assert_eq!(FileFormat::Bson.mime_type(), "application/bson");
    assert_eq!(FileFormat::Parquet.mime_type(), "application/parquet");
}

#[test]
fn test_error_handling() {
    use scarydb::error::{ScaryError, Result};
    
    let err: Result<()> = Err(ScaryError::Database("test error".to_string()));
    assert!(err.is_err());
    assert_eq!(err.unwrap_err().to_string(), "Database error: test error");
    
    let err: Result<()> = Err(ScaryError::NotFound("item not found".to_string()));
    assert!(err.is_err());
}

#[test]
fn test_concurrent_access_simulation() {
    use std::thread;
    use std::time::Duration;
    
    let pool = create_test_worker_pool();
    let mut handles = vec![];
    
    for i in 0..10 {
        let pool = pool.clone();
        let handle = thread::spawn(move || {
            // Simulate concurrent operations
            let cmd = Command::UseDatabase { name: format!("db{}", i) };
            let _ = pool.execute(cmd);
            thread::sleep(Duration::from_millis(1));
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    pool.shutdown();
}

#[test]
fn test_large_document_handling() {
    let mut large_doc = Document::new();
    for i in 0..1000 {
        large_doc.insert(format!("field_{}", i), Bson::Int32(i));
    }
    
    let v = Value::object(large_doc.clone());
    let bson = v.to_bson();
    let v2 = Value::from_bson(bson);
    assert_eq!(v, v2);
}

#[test]
fn test_unicode_support() {
    let doc = doc! { 
        "name": "测试", 
        "emoji": "🎉🚀", 
        "arabic": "مرحبا",
        "hebrew": "שלום"
    };
    
    let v = Value::object(doc.clone());
    let json = v.to_json();
    let v2 = Value::from_json(json).unwrap();
    assert_eq!(v, v2);
    
    // Test with file manager
    let file_manager = FileManager::new();
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();
    
    let export_options = ExportOptions {
        format: FileFormat::Json,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    file_manager.export(path, &[doc.clone()], &export_options).unwrap();
    
    let import_options = ImportOptions {
        format: FileFormat::Json,
        collection: "test".to_string(),
        ..Default::default()
    };
    
    let stats = file_manager.import(path, &import_options).unwrap();
    assert_eq!(stats.imported, 1);
}