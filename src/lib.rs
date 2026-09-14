//! ScaryDB - MongoDB-compatible Document Database
//!
//! A high-performance, in-memory document database with MongoDB-compatible
//! query language, multi-format file import/export, and distributed capabilities.

pub mod core;
pub mod io;
pub mod query;
pub mod storage;
pub mod worker;
pub mod engine;

// Re-export bson's doc macro for convenience
pub use bson::doc;

// Re-export error macros (already exported at crate root via #[macro_export])
// These are available directly as crate::macro_name!

/// Re-export commonly used types
pub use core::document::{Value, DocumentRecord, QueryFilter, UpdateSpec, PipelineStage, IndexDef, CollectionOptions, ValidationLevel, ValidationAction, GridFSFile};
pub use core::error::{ScaryError, Result, ScaryResultExt};
pub use io::file_manager::{FileManager, FileFormat, ImportOptions, ExportOptions, ImportStats, ExportStats};
pub use query::parser::{Parser, Command};
pub use storage::storage::{StorageEngine, StorageConfig, GlobalStats};
pub use worker::worker::{WorkerPool, Response, CommandStats};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the library
pub fn init() -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}