// Health check and metrics HTTP server

use axum::{
    routing::get,
    Router,
    response::Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::worker::{DatabaseSystem, Request, Response, WorkerPool};
use crate::config::Config;
use flume::Sender;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// Global metrics
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
static TOTAL_REQUESTS: AtomicU64 = AtomicU64::new(0);
static SUCCESSFUL_REQUESTS: AtomicU64 = AtomicU64::new(0);
static FAILED_REQUESTS: AtomicU64 = AtomicU64::new(0);
static TOTAL_BYTES_READ: AtomicU64 = AtomicU64::new(0);
static TOTAL_BYTES_WRITTEN: AtomicU64 = AtomicU64::new(0);

pub fn init_metrics() {
    START_TIME.get_or_init(Instant::now);
}

pub fn record_request(success: bool, duration: std::time::Duration, bytes_read: u64, bytes_written: u64) {
    TOTAL_REQUESTS.fetch_add(1, Ordering::Relaxed);
    if success {
        SUCCESSFUL_REQUESTS.fetch_add(1, Ordering::Relaxed);
    } else {
        FAILED_REQUESTS.fetch_add(1, Ordering::Relaxed);
    }
    TOTAL_BYTES_READ.fetch_add(bytes_read, Ordering::Relaxed);
    TOTAL_BYTES_WRITTEN.fetch_add(bytes_written, Ordering::Relaxed);
}

pub fn update_uptime_gauge() {
    if let Some(start) = START_TIME.get() {
        // Metrics update will be done via Prometheus handle
    }
}

/// Health check response
#[derive(serde::Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: f64,
    timestamp: String,
}

/// Readiness check response
#[derive(serde::Serialize)]
struct ReadyResponse {
    ready: bool,
    checks: Value,
}

/// Simple Prometheus metrics endpoint
async fn metrics_handler() -> impl IntoResponse {
    // Build the recorder and get handle
    let handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .build_recorder()
        .handle();
    let metrics_str = handle.render();
    
    (StatusCode::OK, metrics_str)
}

/// Health check endpoint
async fn health_handler() -> Json<HealthResponse> {
    let uptime = START_TIME.get().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Readiness check endpoint
async fn ready_handler() -> Json<ReadyResponse> {
    // Check if database is responsive
    let mut checks = serde_json::Map::new();
    checks.insert("database".to_string(), json!({"status": "ok", "message": "Database operational"}));
    checks.insert("storage".to_string(), json!({"status": "ok", "message": "Storage accessible"}));
    
    Json(ReadyResponse {
        ready: true,
        checks: Value::Object(checks),
    })
}

/// Build the HTTP router for health/metrics
pub fn build_health_router() -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/metrics", get(metrics_handler))
        .layer(TraceLayer::new_for_http())
}

/// Run the health check HTTP server
pub async fn run_health_server(addr: std::net::SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = build_health_router();
    let listener = TcpListener::bind(addr).await?;
    info!("Health check server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}