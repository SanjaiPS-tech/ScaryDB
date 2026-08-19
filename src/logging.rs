// Logging initialization and utilities

use tracing::info;
use tracing::error;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use std::sync::Once;

static LOG_INIT: Once = Once::new();

/// Initialize structured logging with tracing
pub fn init_logging() {
    LOG_INIT.call_once(|| {
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,scarydb=debug"));

        let fmt_layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_target(true);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();

        info!(
            version = env!("CARGO_PKG_VERSION"),
            "ScaryDB logging initialized"
        );
    });
}

/// Initialize logging for tests (silent by default)
pub fn init_test_logging() {
    LOG_INIT.call_once(|| {
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off"));

        let fmt_layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(true);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    });
}

/// Macro for structured logging with correlation IDs
#[macro_export]
macro_rules! log_request {
    ($method:expr, $path:expr, $status:expr, $duration_ms:expr) => {
        info!(
            method = %$method,
            path = %$path,
            status = %$status,
            duration_ms = %$duration_ms,
            "request_completed"
        );
    };
}

/// Macro for database operations
#[macro_export]
macro_rules! log_db_operation {
    ($op:expr, $db:expr, $bucket:expr, $key:expr, $success:expr) => {
        info!(
            operation = %$op,
            database = %$db,
            bucket = %$bucket,
            key = %$key,
            success = %$success,
            "database_operation"
        );
    };
}

/// Macro for error logging
#[macro_export]
macro_rules! log_error {
    ($err:expr, $context:expr) => {
        error!(
            error = %$err,
            context = %$context,
            "operation_failed"
        );
    };
}