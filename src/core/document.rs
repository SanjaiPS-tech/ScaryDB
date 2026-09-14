//! Core document types for ScaryDB
//!
//! Provides a comprehensive document model compatible with MongoDB's BSON
//! with extensions for file format support.

use bson::{Bson, Document, DateTime, Decimal128, oid::ObjectId, Regex, Timestamp};
use chrono::{DateTime as ChronoDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

// Re-export bson's doc macro for convenience
use bson::doc;

/// Extended document value type supporting all BSON types plus format-specific types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "$type", content = "$value")]
pub enum Value {
    /// Null value
    Null,
    /// Boolean
    Bool(bool),
    /// 32-bit integer
    Int32(i32),
    /// 64-bit integer
    Int64(i64),
    /// 64-bit float
    Double(f64),
    /// String
    String(String),
    /// Binary data
    Binary(ByteBuf),
    /// ObjectId
    ObjectId(String),
    /// DateTime (UTC milliseconds since epoch)
    DateTime(i64),
    /// Array of values
    Array(Vec<Value>),
    /// Embedded document
    Object(Document),
    /// Regular expression
    Regex(String, String),
    /// JavaScript code
    JavaScriptCode(String),
    /// JavaScript code with scope
    JavaScriptCodeWithScope(String, Document),
    /// Timestamp
    Timestamp(u32, u32),
    /// 128-bit decimal
    Decimal128(String),
    /// MinKey
    MinKey,
    /// MaxKey
    MaxKey,
    /// Symbol (deprecated in MongoDB)
    Symbol(String),
    /// Undefined (deprecated in MongoDB)
    Undefined,
    /// DBPointer (deprecated in MongoDB)
    DBPointer(String, String),
    /// UUID
    Uuid(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int32(i) => write!(f, "{}", i),
            Value::Int64(i) => write!(f, "{}", i),
            Value::Double(d) => write!(f, "{}", d),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Binary(b) => write!(f, "BinData({}, \"{}\")", b.len(), base64::encode(b)),
            Value::ObjectId(id) => write!(f, "ObjectId(\"{}\")", id),
            Value::DateTime(ts) => {
                let dt = ChronoDateTime::from_timestamp_millis(*ts).unwrap_or_default();
                write!(f, "ISODate(\"{}\")", dt.to_rfc3339())
            }
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Object(doc) => {
                write!(f, "{{")?;
                for (i, (k, v)) in doc.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "\"{}\": {}", k, value_to_string(v))?;
                }
                write!(f, "}}")
            }
            Value::Regex(pattern, options) => write!(f, "/{}/{}", pattern, options),
            Value::JavaScriptCode(code) => write!(f, "Code(\"{}\")", code),
            Value::JavaScriptCodeWithScope(code, _) => write!(f, "CodeWithScope(\"{}\")", code),
            Value::Timestamp(t, i) => write!(f, "Timestamp({}, {})", t, i),
            Value::Decimal128(d) => write!(f, "Decimal128(\"{}\")", d),
            Value::MinKey => write!(f, "MinKey"),
            Value::MaxKey => write!(f, "MaxKey"),
            Value::Symbol(s) => write!(f, "Symbol(\"{}\")", s),
            Value::Undefined => write!(f, "undefined"),
            Value::DBPointer(ns, id) => write!(f, "DBPointer(\"{}\", \"{}\")", ns, id),
            Value::Uuid(u) => write!(f, "UUID(\"{}\")", u),
        }
    }
}

fn value_to_string(bson: &Bson) -> String {
    match bson {
        Bson::Null => "null".to_string(),
        Bson::Boolean(b) => b.to_string(),
        Bson::Int32(i) => i.to_string(),
        Bson::Int64(i) => i.to_string(),
        Bson::Double(d) => d.to_string(),
        Bson::String(s) => format!("\"{}\"", s),
        Bson::Binary(b) => format!("BinData({}, \"{}\")", b.bytes.len(), base64::encode(&b.bytes)),
        Bson::ObjectId(id) => format!("ObjectId(\"{}\")", id),
        Bson::DateTime(dt) => format!("ISODate(\"{}\")", dt.try_to_rfc3339_string().unwrap_or_default()),
        Bson::Array(arr) => {
            let items: Vec<String> = arr.iter().map(value_to_string).collect();
            format!("[{}]", items.join(", "))
        }
        Bson::Document(doc) => {
            let items: Vec<String> = doc.iter().map(|(k, v)| format!("\"{}\": {}", k, value_to_string(v))).collect();
            format!("{{{}}}", items.join(", "))
        }
        Bson::RegularExpression(re) => format!("/{}/{}", re.pattern, re.options),
        Bson::JavaScriptCode(js) => format!("Code(\"{}\")", js),
        Bson::JavaScriptCodeWithScope(js) => format!("CodeWithScope(\"{}\")", js.code),
        Bson::Timestamp(ts) => format!("Timestamp({}, {})", ts.time, ts.increment),
        Bson::Decimal128(d) => format!("Decimal128(\"{}\")", d),
        Bson::MinKey => "MinKey".to_string(),
        Bson::MaxKey => "MaxKey".to_string(),
        Bson::Symbol(s) => format!("Symbol(\"{}\")", s),
        Bson::Undefined => "undefined".to_string(),
        Bson::DbPointer(dp) => format!("DBPointer(\"{}\", \"{}\")", dp.namespace, dp.id),
        _ => format!("{:?}", bson),
    }
}

impl Value {
    /// Create a new null value
    pub fn null() -> Self {
        Value::Null
    }
    
    /// Create a new boolean value
    pub fn bool(b: bool) -> Self {
        Value::Bool(b)
    }
    
    /// Create a new int32 value
    pub fn int32(i: i32) -> Self {
        Value::Int32(i)
    }
    
    /// Create a new int64 value
    pub fn int64(i: i64) -> Self {
        Value::Int64(i)
    }
    
    /// Create a new double value
    pub fn double(d: f64) -> Self {
        Value::Double(d)
    }
    
    /// Create a new string value
    pub fn string(s: impl Into<String>) -> Self {
        Value::String(s.into())
    }
    
    /// Create a new binary value
    pub fn binary(data: Vec<u8>) -> Self {
        Value::Binary(ByteBuf::from(data))
    }
    
    /// Create a new ObjectId
    pub fn object_id(id: impl Into<String>) -> Self {
        Value::ObjectId(id.into())
    }
    
    /// Create a new datetime (milliseconds since epoch)
    pub fn datetime(ts: i64) -> Self {
        Value::DateTime(ts)
    }
    
    /// Create a new datetime from chrono
    pub fn datetime_chrono(dt: ChronoDateTime<Utc>) -> Self {
        Value::DateTime(dt.timestamp_millis())
    }
    
    /// Create a new array
    pub fn array(arr: Vec<Value>) -> Self {
        Value::Array(arr)
    }
    
    /// Create a new object
    pub fn object(doc: Document) -> Self {
        Value::Object(doc)
    }
    
    /// Create a new regex
    pub fn regex(pattern: impl Into<String>, options: impl Into<String>) -> Self {
        Value::Regex(pattern.into(), options.into())
    }
    
    /// Create a new UUID
    pub fn uuid(u: impl Into<String>) -> Self {
        Value::Uuid(u.into())
    }
    
    /// Get the BSON type name
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int32(_) => "int32",
            Value::Int64(_) => "int64",
            Value::Double(_) => "double",
            Value::String(_) => "string",
            Value::Binary(_) => "binary",
            Value::ObjectId(_) => "objectId",
            Value::DateTime(_) => "date",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Regex(_, _) => "regex",
            Value::JavaScriptCode(_) => "javascriptCode",
            Value::JavaScriptCodeWithScope(_, _) => "javascriptCodeWithScope",
            Value::Timestamp(_, _) => "timestamp",
            Value::Decimal128(_) => "decimal128",
            Value::MinKey => "minKey",
            Value::MaxKey => "maxKey",
            Value::Symbol(_) => "symbol",
            Value::Undefined => "undefined",
            Value::DBPointer(_, _) => "dbPointer",
            Value::Uuid(_) => "uuid",
        }
    }
    
    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    
    /// Check if value is a number
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Int32(_) | Value::Int64(_) | Value::Double(_))
    }
    
    /// Check if value is a string
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }
    
    /// Check if value is an array
    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }
    
    /// Check if value is an object
    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }
    
    /// Get as string if possible
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    
    /// Get as i64 if possible
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int32(i) => Some(*i as i64),
            Value::Int64(i) => Some(*i),
            Value::Double(d) => Some(*d as i64),
            _ => None,
        }
    }
    
    /// Get as f64 if possible
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int32(i) => Some(*i as f64),
            Value::Int64(i) => Some(*i as f64),
            Value::Double(d) => Some(*d),
            _ => None,
        }
    }
    
    /// Get as bool if possible
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
    
    /// Get array length
    pub fn array_len(&self) -> Option<usize> {
        match self {
            Value::Array(arr) => Some(arr.len()),
            _ => None,
        }
    }
    
    /// Get object keys
    pub fn object_keys(&self) -> Option<Vec<String>> {
        match self {
            Value::Object(doc) => Some(doc.keys().cloned().collect()),
            _ => None,
        }
    }
    
    /// Convert to BSON
    pub fn to_bson(&self) -> Bson {
        match self {
            Value::Null => Bson::Null,
            Value::Bool(b) => Bson::Boolean(*b),
            Value::Int32(i) => Bson::Int32(*i),
            Value::Int64(i) => Bson::Int64(*i),
            Value::Double(d) => Bson::Double(*d),
            Value::String(s) => Bson::String(s.clone()),
            Value::Binary(b) => Bson::Binary(bson::Binary {
                subtype: bson::spec::BinarySubtype::Generic,
                bytes: b.clone().into(),
            }),
            Value::ObjectId(id) => Bson::ObjectId(ObjectId::parse_str(id).unwrap_or_default()),
            Value::DateTime(ts) => Bson::DateTime(DateTime::from_millis(*ts)),
            Value::Array(arr) => Bson::Array(arr.iter().map(|v| v.to_bson()).collect()),
            Value::Object(doc) => Bson::Document(doc.clone()),
            Value::Regex(pattern, options) => Bson::RegularExpression(Regex {
                pattern: pattern.clone(),
                options: options.clone(),
            }),
            Value::JavaScriptCode(code) => Bson::JavaScriptCode(code.clone()),
            Value::JavaScriptCodeWithScope(code, scope) => Bson::JavaScriptCodeWithScope(
                bson::JavaScriptCodeWithScope {
                    code: code.clone(),
                    scope: scope.clone(),
                }
            ),
            Value::Timestamp(t, i) => Bson::Timestamp(Timestamp { time: *t, increment: *i }),
            Value::Decimal128(d) => Bson::Decimal128(Decimal128::from_str(d).unwrap_or_default()),
            Value::MinKey => Bson::MinKey,
            Value::MaxKey => Bson::MaxKey,
            Value::Symbol(s) => Bson::String(format!("Symbol(\"{}\")", s)),
            Value::Undefined => Bson::String("undefined".to_string()),
            Value::DBPointer(ns, id) => Bson::String(format!("DBPointer(\"{}\", \"{}\")", ns, id)),
            Value::Uuid(u) => Bson::String(u.clone()), // Store UUID as string
        }
    }
    
    /// Create from BSON
    pub fn from_bson(bson: Bson) -> Self {
        match bson {
            Bson::Null => Value::Null,
            Bson::Boolean(b) => Value::Bool(b),
            Bson::Int32(i) => Value::Int32(i),
            Bson::Int64(i) => Value::Int64(i),
            Bson::Double(d) => Value::Double(d),
            Bson::String(s) => Value::String(s),
            Bson::Binary(b) => Value::Binary(ByteBuf::from(b.bytes.to_vec())),
            Bson::ObjectId(id) => Value::ObjectId(id.to_hex()),
            Bson::DateTime(dt) => Value::DateTime(dt.timestamp_millis()),
            Bson::Array(arr) => Value::Array(arr.into_iter().map(Value::from_bson).collect()),
            Bson::Document(doc) => Value::Object(doc),
            Bson::RegularExpression(re) => Value::Regex(re.pattern, re.options),
            Bson::JavaScriptCode(js) => Value::JavaScriptCode(js),
            Bson::JavaScriptCodeWithScope(js) => Value::JavaScriptCodeWithScope(js.code, js.scope),
            Bson::Timestamp(ts) => Value::Timestamp(ts.time, ts.increment),
            Bson::Decimal128(d) => Value::Decimal128(d.to_string()),
            Bson::MinKey => Value::MinKey,
            Bson::MaxKey => Value::MaxKey,
            Bson::Symbol(s) => Value::Symbol(s),
            Bson::Undefined => Value::Undefined,
            Bson::DbPointer(dp) => Value::DBPointer(dp.namespace.to_string(), dp.id.to_hex()),
        }
    }
    
    /// Convert from JSON value
    pub fn from_json(value: serde_json::Value) -> Result<Self, String> {
        Ok(match value {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => {
                if n.is_i64() {
                    Value::Int64(n.as_i64().unwrap())
                } else if n.is_u64() {
                    Value::Int64(n.as_u64().unwrap() as i64)
                } else {
                    Value::Double(n.as_f64().unwrap())
                }
            }
            serde_json::Value::String(s) => Value::String(s),
            serde_json::Value::Array(arr) => Value::Array(arr.into_iter().map(Value::from_json).collect::<Result<Vec<_>, _>>()?),
            serde_json::Value::Object(obj) => {
                let mut doc = Document::new();
                for (k, v) in obj {
                    doc.insert(k, Value::from_json(v)?.to_bson());
                }
                Value::Object(doc)
            }
        })
    }
    
    /// Convert to JSON value
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Int32(i) => serde_json::Value::Number((*i).into()),
            Value::Int64(i) => serde_json::Value::Number((*i).into()),
            Value::Double(d) => serde_json::Value::Number(
                serde_json::Number::from_f64(*d).unwrap_or(serde_json::Number::from(0))
            ),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Binary(b) => serde_json::Value::String(base64::encode(b)),
            Value::ObjectId(id) => serde_json::Value::String(id.clone()),
            Value::DateTime(ts) => {
                let dt = ChronoDateTime::from_timestamp_millis(*ts).unwrap_or_default();
                serde_json::Value::String(dt.to_rfc3339())
            }
            Value::Array(arr) => serde_json::Value::Array(arr.iter().map(|v| v.to_json()).collect()),
            Value::Object(doc) => {
                let mut map = serde_json::Map::new();
                for (k, v) in doc {
                    map.insert(k.clone(), Value::from_bson(v.clone()).to_json());
                }
                serde_json::Value::Object(map)
            }
            Value::Regex(p, o) => serde_json::Value::String(format!("/{}/{}", p, o)),
            Value::JavaScriptCode(c) => serde_json::Value::String(format!("Code(\"{}\")", c)),
            Value::JavaScriptCodeWithScope(c, _) => serde_json::Value::String(format!("CodeWithScope(\"{}\")", c)),
            Value::Timestamp(t, i) => serde_json::Value::String(format!("Timestamp({}, {})", t, i)),
            Value::Decimal128(d) => serde_json::Value::String(format!("Decimal128(\"{}\")", d)),
            Value::MinKey => serde_json::Value::String("MinKey".to_string()),
            Value::MaxKey => serde_json::Value::String("MaxKey".to_string()),
            Value::Symbol(s) => serde_json::Value::String(format!("Symbol(\"{}\")", s)),
            Value::Undefined => serde_json::Value::String("undefined".to_string()),
            Value::DBPointer(ns, id) => serde_json::Value::String(format!("DBPointer(\"{}\", \"{}\")", ns, id)),
            Value::Uuid(u) => serde_json::Value::String(u.clone()),
        }
    }
}

/// Document wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub _id: Value,
    pub data: Document,
    pub created_at: i64,
    pub updated_at: i64,
    pub version: u64,
}

impl DocumentRecord {
    pub fn new(data: Document) -> Self {
        let now = Utc::now().timestamp_millis();
        let id = data.get("_id")
            .map(|v| Value::from_bson(v.clone()))
            .unwrap_or_else(|| Value::object_id(ObjectId::new().to_hex()));
        
        Self {
            _id: id,
            data,
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }
    
    pub fn with_id(id: Value, data: Document) -> Self {
        let now = Utc::now().timestamp_millis();
        Self {
            _id: id,
            data,
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }
    
    pub fn get_id(&self) -> &Value {
        &self._id
    }
    
    pub fn get_data(&self) -> &Document {
        &self.data
    }
    
    pub fn get_data_mut(&mut self) -> &mut Document {
        self.updated_at = Utc::now().timestamp_millis();
        self.version += 1;
        &mut self.data
    }
    
    pub fn to_document(&self) -> Document {
        let mut doc = self.data.clone();
        doc.insert("_id", self._id.to_bson());
        doc.insert("_created_at", Bson::DateTime(DateTime::from_millis(self.created_at)));
        doc.insert("_updated_at", Bson::DateTime(DateTime::from_millis(self.updated_at)));
        doc.insert("_version", Bson::Int64(self.version as i64));
        doc
    }
}

/// Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDef {
    pub name: String,
    pub keys: Document,
    pub unique: bool,
    pub sparse: bool,
    pub background: bool,
    pub expire_after_seconds: Option<u32>,
    pub partial_filter: Option<Document>,
    pub weights: Option<Document>,
    pub default_language: Option<String>,
    pub language_override: Option<String>,
    pub text_version: Option<u32>,
    pub sphere_version: Option<u32>,
    pub bits: Option<u8>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub bucket_size: Option<f64>,
}

impl IndexDef {
    pub fn new(name: impl Into<String>, keys: Document) -> Self {
        Self {
            name: name.into(),
            keys,
            unique: false,
            sparse: false,
            background: true,
            expire_after_seconds: None,
            partial_filter: None,
            weights: None,
            default_language: None,
            language_override: None,
            text_version: None,
            sphere_version: None,
            bits: None,
            min: None,
            max: None,
            bucket_size: None,
        }
    }
    
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }
    
    pub fn sparse(mut self) -> Self {
        self.sparse = true;
        self
    }
    
    pub fn ttl(mut self, seconds: u32) -> Self {
        self.expire_after_seconds = Some(seconds);
        self
    }
    
    pub fn partial_filter(mut self, filter: Document) -> Self {
        self.partial_filter = Some(filter);
        self
    }
}

/// Collection options
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectionOptions {
    pub capped: bool,
    pub size: Option<u64>,
    pub max: Option<u64>,
    pub validator: Option<Document>,
    pub validation_level: ValidationLevel,
    pub validation_action: ValidationAction,
    pub index_option_defaults: Option<Document>,
    pub view_on: Option<String>,
    pub pipeline: Option<Vec<Document>>,
    pub collation: Option<Document>,
    pub write_concern: Option<Document>,
    pub read_concern: Option<Document>,
    pub read_preference: Option<Document>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ValidationLevel {
    #[default]
    Off,
    Strict,
    Moderate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ValidationAction {
    #[default]
    Error,
    Warn,
}

/// GridFS file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridFSFile {
    pub _id: Value,
    pub length: i64,
    pub chunk_size: i32,
    pub upload_date: i64,
    pub md5: Option<String>,
    pub filename: String,
    pub content_type: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub metadata: Option<Document>,
}

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    Active,
    Committed,
    Aborted,
}

/// Aggregation pipeline stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStage {
    pub stage: String,
    pub spec: Document,
}

impl PipelineStage {
    pub fn match_stage(filter: Document) -> Self {
        Self { stage: "$match".to_string(), spec: filter }
    }
    
    pub fn project_stage(spec: Document) -> Self {
        Self { stage: "$project".to_string(), spec }
    }
    
    pub fn group_stage(spec: Document) -> Self {
        Self { stage: "$group".to_string(), spec }
    }
    
    pub fn sort_stage(spec: Document) -> Self {
        Self { stage: "$sort".to_string(), spec }
    }
    
    pub fn limit_stage(limit: i64) -> Self {
        Self { stage: "$limit".to_string(), spec: doc! { "limit": limit } }
    }
    
    pub fn skip_stage(skip: i64) -> Self {
        Self { stage: "$skip".to_string(), spec: doc! { "skip": skip } }
    }
    
    pub fn unwind_stage(path: impl Into<String>, preserve_null: bool) -> Self {
        Self { 
            stage: "$unwind".to_string(), 
            spec: doc! { 
                "path": path.into(),
                "preserveNullAndEmptyArrays": preserve_null
            }
        }
    }
    
    pub fn lookup_stage(from: impl Into<String>, local_field: impl Into<String>, foreign_field: impl Into<String>, as_field: impl Into<String>) -> Self {
        Self {
            stage: "$lookup".to_string(),
            spec: doc! {
                "from": from.into(),
                "localField": local_field.into(),
                "foreignField": foreign_field.into(),
                "as": as_field.into()
            }
        }
    }
    
    pub fn add_fields_stage(spec: Document) -> Self {
        Self { stage: "$addFields".to_string(), spec }
    }
    
    pub fn replace_root_stage(new_root: Document) -> Self {
        Self { stage: "$replaceRoot".to_string(), spec: doc! { "newRoot": new_root } }
    }
    
    pub fn count_stage(as_field: impl Into<String>) -> Self {
        Self { stage: "$count".to_string(), spec: doc! { "as": as_field.into() } }
    }
    
    pub fn facet_stage(facets: Document) -> Self {
        Self { stage: "$facet".to_string(), spec: facets }
    }
    
    pub fn bucket_stage(group_by: impl Into<String>, boundaries: Vec<Value>, default: Option<Value>, output: Option<Document>) -> Self {
        let mut spec = doc! {
            "groupBy": group_by.into(),
            "boundaries": boundaries.into_iter().map(|v| v.to_bson()).collect::<Vec<_>>()
        };
        if let Some(def) = default {
            spec.insert("default", def.to_bson());
        }
        if let Some(out) = output {
            spec.insert("output", out);
        }
        Self { stage: "$bucket".to_string(), spec }
    }
    
    pub fn graph_lookup_stage(from: impl Into<String>, start_with: impl Into<String>, connect_from: impl Into<String>, connect_to: impl Into<String>, as_field: impl Into<String>, max_depth: Option<i64>, depth_field: Option<String>, restrict_search: Option<Document>) -> Self {
        let mut spec = doc! {
            "from": from.into(),
            "startWith": start_with.into(),
            "connectFromField": connect_from.into(),
            "connectToField": connect_to.into(),
            "as": as_field.into()
        };
        if let Some(d) = max_depth {
            spec.insert("maxDepth", Bson::Int64(d));
        }
        if let Some(df) = depth_field {
            spec.insert("depthField", df);
        }
        if let Some(rs) = restrict_search {
            spec.insert("restrictSearchWithMatch", rs);
        }
        Self { stage: "$graphLookup".to_string(), spec }
    }
}

/// Query filter builder
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryFilter {
    pub filter: Document,
}

impl QueryFilter {
    pub fn new(filter: Document) -> Self {
        Self { filter }
    }
    
    pub fn empty() -> Self {
        Self { filter: Document::new() }
    }
    
    pub fn eq(field: impl Into<String>, value: Value) -> Self {
        Self { filter: doc! { field.into(): value.to_bson() } }
    }
    
    pub fn ne(field: impl Into<String>, value: Value) -> Self {
        Self { filter: doc! { field.into(): { "$ne": value.to_bson() } } }
    }
    
    pub fn gt(field: impl Into<String>, value: Value) -> Self {
        Self { filter: doc! { field.into(): { "$gt": value.to_bson() } } }
    }
    
    pub fn gte(field: impl Into<String>, value: Value) -> Self {
        Self { filter: doc! { field.into(): { "$gte": value.to_bson() } } }
    }
    
    pub fn lt(field: impl Into<String>, value: Value) -> Self {
        Self { filter: doc! { field.into(): { "$lt": value.to_bson() } } }
    }
    
    pub fn lte(field: impl Into<String>, value: Value) -> Self {
        Self { filter: doc! { field.into(): { "$lte": value.to_bson() } } }
    }
    
    pub fn in_array(field: impl Into<String>, values: Vec<Value>) -> Self {
        Self { filter: doc! { field.into(): { "$in": values.into_iter().map(|v| v.to_bson()).collect::<Vec<_>>() } } }
    }
    
    pub fn nin(field: impl Into<String>, values: Vec<Value>) -> Self {
        Self { filter: doc! { field.into(): { "$nin": values.into_iter().map(|v| v.to_bson()).collect::<Vec<_>>() } } }
    }
    
    pub fn exists(field: impl Into<String>, exists: bool) -> Self {
        Self { filter: doc! { field.into(): { "$exists": exists } } }
    }
    
    pub fn regex(field: impl Into<String>, pattern: impl Into<String>, options: Option<String>) -> Self {
        let mut spec = doc! { "$regex": pattern.into() };
        if let Some(opts) = options {
            spec.insert("$options", opts);
        }
        Self { filter: doc! { field.into(): spec } }
    }
    
    pub fn text(search: impl Into<String>, language: Option<String>, case_sensitive: Option<bool>, diacritic_sensitive: Option<bool>) -> Self {
        let mut spec = doc! { "$search": search.into() };
        if let Some(lang) = language {
            spec.insert("$language", lang);
        }
        if let Some(cs) = case_sensitive {
            spec.insert("$caseSensitive", cs);
        }
        if let Some(ds) = diacritic_sensitive {
            spec.insert("$diacriticSensitive", ds);
        }
        Self { filter: doc! { "$text": spec } }
    }
    
    pub fn where_js(code: impl Into<String>) -> Self {
        Self { filter: doc! { "$where": code.into() } }
    }
    
    pub fn and(filters: Vec<QueryFilter>) -> Self {
        Self { filter: doc! { "$and": filters.into_iter().map(|f| f.filter).collect::<Vec<_>>() } }
    }
    
    pub fn or(filters: Vec<QueryFilter>) -> Self {
        Self { filter: doc! { "$or": filters.into_iter().map(|f| f.filter).collect::<Vec<_>>() } }
    }
    
    pub fn nor(filters: Vec<QueryFilter>) -> Self {
        Self { filter: doc! { "$nor": filters.into_iter().map(|f| f.filter).collect::<Vec<_>>() } }
    }
    
    pub fn not(filter: QueryFilter) -> Self {
        Self { filter: doc! { "$not": filter.filter } }
    }
    
    pub fn elem_match(field: impl Into<String>, filter: QueryFilter) -> Self {
        Self { filter: doc! { field.into(): { "$elemMatch": filter.filter } } }
    }
    
    pub fn size(field: impl Into<String>, size: i64) -> Self {
        Self { filter: doc! { field.into(): { "$size": size } } }
    }
    
    pub fn all(field: impl Into<String>, values: Vec<Value>) -> Self {
        Self { filter: doc! { field.into(): { "$all": values.into_iter().map(|v| v.to_bson()).collect::<Vec<_>>() } } }
    }
    
    pub fn matches(&self, doc: &Document) -> bool {
        // Simplified matching - in production would use a proper query engine
        for (key, expected) in &self.filter {
            match doc.get(key) {
                Some(actual) if bson_values_equal(actual, expected) => continue,
                _ => return false,
            }
        }
        true
    }
}

pub fn bson_values_equal(a: &Bson, b: &Bson) -> bool {
    match (a, b) {
        (Bson::Null, Bson::Null) => true,
        (Bson::Boolean(a), Bson::Boolean(b)) => a == b,
        (Bson::Int32(a), Bson::Int32(b)) => a == b,
        (Bson::Int64(a), Bson::Int64(b)) => a == b,
        (Bson::Double(a), Bson::Double(b)) => a == b,
        (Bson::String(a), Bson::String(b)) => a == b,
        (Bson::ObjectId(a), Bson::ObjectId(b)) => a == b,
        (Bson::DateTime(a), Bson::DateTime(b)) => a == b,
        (Bson::Array(a), Bson::Array(b)) => {
            if a.len() != b.len() { return false; }
            a.iter().zip(b.iter()).all(|(x, y)| bson_values_equal(x, y))
        }
        (Bson::Document(a), Bson::Document(b)) => {
            if a.len() != b.len() { return false; }
            for (k, v) in a {
                match b.get(k) {
                    Some(v2) if bson_values_equal(v, v2) => continue,
                    _ => return false,
                }
            }
            true
        }
        _ => false,
    }
}

/// Update operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSpec {
    pub operations: Document,
}

impl UpdateSpec {
    pub fn new() -> Self {
        Self { operations: Document::new() }
    }
    
    pub fn set(mut self, field: impl Into<String>, value: Value) -> Self {
        self.operations.entry("$set".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$set").unwrap() {
            doc.insert(field.into(), value.to_bson());
        }
        self
    }
    
    pub fn unset(mut self, field: impl Into<String>) -> Self {
        self.operations.entry("$unset".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$unset").unwrap() {
            doc.insert(field.into(), Bson::String("".to_string()));
        }
        self
    }
    
    pub fn inc(mut self, field: impl Into<String>, value: i64) -> Self {
        self.operations.entry("$inc".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$inc").unwrap() {
            doc.insert(field.into(), Bson::Int64(value));
        }
        self
    }
    
    pub fn mul(mut self, field: impl Into<String>, value: f64) -> Self {
        self.operations.entry("$mul".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$mul").unwrap() {
            doc.insert(field.into(), Bson::Double(value));
        }
        self
    }
    
    pub fn rename(mut self, field: impl Into<String>, new_name: impl Into<String>) -> Self {
        self.operations.entry("$rename".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$rename").unwrap() {
            doc.insert(field.into(), Bson::String(new_name.into()));
        }
        self
    }
    
    pub fn set_on_insert(mut self, field: impl Into<String>, value: Value) -> Self {
        self.operations.entry("$setOnInsert".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$setOnInsert").unwrap() {
            doc.insert(field.into(), value.to_bson());
        }
        self
    }
    
    pub fn min(mut self, field: impl Into<String>, value: Value) -> Self {
        self.operations.entry("$min".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$min").unwrap() {
            doc.insert(field.into(), value.to_bson());
        }
        self
    }
    
    pub fn max(mut self, field: impl Into<String>, value: Value) -> Self {
        self.operations.entry("$max".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$max").unwrap() {
            doc.insert(field.into(), value.to_bson());
        }
        self
    }
    
    pub fn current_date(mut self, field: impl Into<String>, date_type: Option<String>) -> Self {
        self.operations.entry("$currentDate".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$currentDate").unwrap() {
            if let Some(t) = date_type {
                doc.insert(field.into(), Bson::String(t));
            } else {
                doc.insert(field.into(), Bson::Boolean(true));
            }
        }
        self
    }
    
    pub fn push(mut self, field: impl Into<String>, value: Value) -> Self {
        self.operations.entry("$push".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$push").unwrap() {
            doc.insert(field.into(), value.to_bson());
        }
        self
    }
    
    pub fn push_each(mut self, field: impl Into<String>, values: Vec<Value>, slice: Option<i64>, sort: Option<Document>, position: Option<i64>) -> Self {
        let mut push_doc = doc! { "$each": values.into_iter().map(|v| v.to_bson()).collect::<Vec<_>>() };
        if let Some(s) = slice { push_doc.insert("$slice", s); }
        if let Some(s) = sort { push_doc.insert("$sort", s); }
        if let Some(p) = position { push_doc.insert("$position", p); }
        
        self.operations.entry("$push".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$push").unwrap() {
            doc.insert(field.into(), Bson::Document(push_doc));
        }
        self
    }
    
    pub fn add_to_set(mut self, field: impl Into<String>, value: Value) -> Self {
        self.operations.entry("$addToSet".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$addToSet").unwrap() {
            doc.insert(field.into(), value.to_bson());
        }
        self
    }
    
    pub fn add_to_set_each(mut self, field: impl Into<String>, values: Vec<Value>) -> Self {
        let mut set_doc = doc! { "$each": values.into_iter().map(|v| v.to_bson()).collect::<Vec<_>>() };
        self.operations.entry("$addToSet".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$addToSet").unwrap() {
            doc.insert(field.into(), Bson::Document(set_doc));
        }
        self
    }
    
    pub fn pop(mut self, field: impl Into<String>, first: bool) -> Self {
        self.operations.entry("$pop".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$pop").unwrap() {
            doc.insert(field.into(), if first { Bson::Int32(-1) } else { Bson::Int32(1) });
        }
        self
    }
    
    pub fn pull(mut self, field: impl Into<String>, value: Value) -> Self {
        self.operations.entry("$pull".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$pull").unwrap() {
            doc.insert(field.into(), value.to_bson());
        }
        self
    }
    
    pub fn pull_all(mut self, field: impl Into<String>, values: Vec<Value>) -> Self {
        let pull_doc = Bson::Array(values.into_iter().map(|v| v.to_bson()).collect());
        self.operations.entry("$pullAll".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$pullAll").unwrap() {
            doc.insert(field.into(), pull_doc);
        }
        self
    }
    
    pub fn bit_and(mut self, field: impl Into<String>, value: i64) -> Self {
        let mut bit_doc = doc! { "and": value };
        self.operations.entry("$bit".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$bit").unwrap() {
            doc.insert(field.into(), Bson::Document(bit_doc));
        }
        self
    }
    
    pub fn bit_or(mut self, field: impl Into<String>, value: i64) -> Self {
        let mut bit_doc = doc! { "or": value };
        self.operations.entry("$bit".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$bit").unwrap() {
            doc.insert(field.into(), Bson::Document(bit_doc));
        }
        self
    }
    
    pub fn bit_xor(mut self, field: impl Into<String>, value: i64) -> Self {
        let mut bit_doc = doc! { "xor": value };
        self.operations.entry("$bit".to_string()).or_insert_with(|| Bson::Document(Document::new()));
        if let Bson::Document(ref mut doc) = self.operations.get_mut("$bit").unwrap() {
            doc.insert(field.into(), Bson::Document(bit_doc));
        }
        self
    }
}

impl Default for UpdateSpec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;
    
    #[test]
    fn test_value_creation() {
        let v = Value::string("test");
        assert_eq!(v.type_name(), "string");
        assert!(v.is_string());
        
        let v = Value::int64(42);
        assert_eq!(v.type_name(), "int64");
        assert!(v.is_number());
        assert_eq!(v.as_i64(), Some(42));
    }
    
    #[test]
    fn test_bson_conversion() {
        let v = Value::string("hello");
        let bson = v.to_bson();
        let v2 = Value::from_bson(bson);
        assert_eq!(v, v2);
        
        let v = Value::object(doc! {"a": 1, "b": "test"});
        let bson = v.to_bson();
        let v2 = Value::from_bson(bson);
        assert_eq!(v, v2);
    }
    
    #[test]
    fn test_query_filter() {
        let filter = QueryFilter::eq("name", Value::string("John"))
            .eq("age", Value::int32(30));
        let doc = doc! { "name": "John", "age": 30, "city": "NYC" };
        assert!(filter.matches(&doc));
        
        let filter = QueryFilter::gt("age", Value::int32(25));
        let doc = doc! { "age": 30 };
        assert!(filter.matches(&doc));
    }
    
    #[test]
    fn test_update_spec() {
        let update = UpdateSpec::new()
            .set("name", Value::string("Jane"))
            .inc("age", 1)
            .set_on_insert("created_at", Value::datetime_chrono(Utc::now()));
        
        let bson = update.operations;
        assert!(bson.contains_key("$set"));
        assert!(bson.contains_key("$inc"));
        assert!(bson.contains_key("$setOnInsert"));
    }
}