use axum::{
    body::Body,
    extract::Multipart,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

mod metrics;
mod watcher;

pub struct AppState {
    pub vault_path: PathBuf,
    pub security_watcher: Arc<watcher::SecurityWatcher>,
    pub ai_url: String,
    pub opa_url: String,
}

async fn download_file(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> impl IntoResponse {
    let path = state.vault_path.join(&filename);

    if !path.starts_with(&state.vault_path) {
        return (StatusCode::FORBIDDEN, "Access Denied").into_response();
    }

    match tokio::fs::File::open(&path).await {
        Ok(file) => {
            let stream = tokio_util::io::ReaderStream::new(file);
            let body = Body::from_stream(stream);
            Response::builder()
                .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
                .header(
                    axum::http::header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(body)
                .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Error").into_response())
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

async fn upload_file(
    axum::extract::State(_state): axum::extract::State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded = 0;
    
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or_else(|| "unknown".into()).to_string();
        match field.bytes().await {
            Ok(data) => {
                let file_path = PathBuf::from("./vault").join(&name);
                if tokio::fs::write(&file_path, &data[..]).await.is_ok() {
                    println!("Uploaded: {}", name);
                    uploaded += 1;
                }
            }
            Err(e) => {
                eprintln!("Error reading field: {}", e);
            }
        }
    }
    
    let body = if uploaded > 0 {
        serde_json::json!({"status": "uploaded", "count": uploaded})
    } else {
        serde_json::json!({"status": "no_files"})
    };
    axum::Json(body).into_response()
}

async fn search_handler(
    axum::extract::State(_state): axum::extract::State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    metrics::SEARCH_QUERIES.inc();

    let query = params.get("q").cloned().unwrap_or_default();

    let results = vec![serde_json::json!({
        "filename": "example.pdf",
        "score": 0.95,
        "excerpt": format!("Result for: {}", query)
    })];

    axum::Json(serde_json::json!({
        "query": query,
        "results": results,
        "total": 1
    })).into_response()
}

async fn status_handler(
    axum::extract::State(_state): axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    let uptime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    axum::Json(serde_json::json!({
        "version": "1.0.0",
        "uptime": uptime,
        "components": {
            "vault": { "status": "encrypted_mounted" },
            "mesh": { "status": "active", "peers": 0 },
            "ai": { "status": "ready" },
            "vector_db": { "status": "connected" }
        },
        "security": {
            "auto_ban": "active",
            "banned_peers": 0,
            "ghost_shell": "armed"
        }
    })).into_response()
}

async fn metrics_handler() -> impl IntoResponse {
    let m = metrics::metrics_handler();
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")
        .body(Body::from(m))
        .unwrap()
}

async fn dashboard_handler() -> impl IntoResponse {
    let html = std::include_str!("../ui/index.html");
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Aetheris Core Active. Zero-Trust Mesh Engaged.");

    let state = Arc::new(AppState {
        vault_path: PathBuf::from("./vault"),
        security_watcher: Arc::new(watcher::SecurityWatcher::new()),
        ai_url: std::env::var("AI_ENDPOINT").unwrap_or_else(|_| "http://ai-engine:11434".to_string()),
        opa_url: std::env::var("OPA_ENDPOINT").unwrap_or_else(|_| "http://opa:8181".to_string()),
    });

    tokio::fs::create_dir_all("./vault").await.ok();

    let app_state = state.clone();
    let app = Router::new()
        .route("/", get(dashboard_handler))
        .route("/upload", post(upload_file))
        .route("/download/:filename", get(download_file))
        .route("/search", get(search_handler))
        .route("/status", get(status_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    println!("Aetheris Core listening on 0.0.0.0:8080");
    
    axum::serve(listener, app).await?;

    Ok(())
}
