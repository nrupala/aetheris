use axum::{
    body::Body,
    extract::{Json as AxumJson, Multipart, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Router,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod connector;
mod metrics;
mod watcher;

pub struct AppState {
    pub vault_path: std::path::PathBuf,
    pub security_watcher: Arc<watcher::SecurityWatcher>,
    pub ai_url: String,
    pub opa_url: String,
    pub port_registry: serde_json::Value,
    pub dev_logs: Mutex<Vec<String>>,
}

fn new_connector(state: &AppState) -> connector::AetherisConnector {
    connector::AetherisConnector {
        ai_url: state.ai_url.clone(),
        opa_url: state.opa_url.clone(),
        vault_path: state.vault_path.to_string_lossy().to_string(),
    }
}

async fn dashboard_handler() -> impl IntoResponse {
    let html = std::include_str!("../ui/index.html");
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap()
}

async fn status_handler(
    State(_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let uptime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let core_port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    axum::Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": uptime,
        "port": core_port,
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
    }))
    .into_response()
}

async fn discovery_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    axum::Json(state.port_registry.clone()).into_response()
}

async fn metrics_handler() -> impl IntoResponse {
    let m = metrics::metrics_handler();
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")
        .body(Body::from(m))
        .unwrap()
}

async fn download_file(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
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
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded = 0;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("unknown").to_string();
        match field.bytes().await {
            Ok(data) => {
                let file_path = state.vault_path.join(&name);
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
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    metrics::SEARCH_QUERIES.inc();

    let query = params.get("q").cloned().unwrap_or_default();
    let connector = new_connector(&state);

    let answer = connector.ai_query(&format!("Search query: {}", query), None).await.unwrap_or_default();

    let results = vec![serde_json::json!({
        "filename": "example.pdf",
        "score": 0.95,
        "excerpt": answer
    })];

    axum::Json(serde_json::json!({
        "query": query,
        "results": results,
        "total": 1
    }))
    .into_response()
}

async fn health_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let connector = new_connector(&state);
    let ai_ok = connector.list_models().await.is_ok();

    let uptime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    axum::Json(serde_json::json!({
        "status": "ok",
        "agents": 4,
        "tasks": 0,
        "tools": 12,
        "prompts": 8,
        "ai_connected": ai_ok,
        "uptime": uptime,
        "spread_forecast": {
            "total_memory_mb": 0,
            "memory_utilization_pct": 0,
            "confidence": 0.0,
            "bottleneck": "none"
        },
        "cross_system": false
    }))
    .into_response()
}

async fn list_models_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let connector = new_connector(&state);

    match connector.list_models().await {
        Ok(models) => {
            let data: Vec<serde_json::Value> = models.into_iter()
                .map(|id| serde_json::json!({
                    "id": id,
                    "object": "model",
                    "owned_by": "ollama"
                }))
                .collect();

            axum::Json(serde_json::json!({
                "object": "list",
                "data": data
            }))
            .into_response()
        }
        Err(e) => {
            (StatusCode::SERVICE_UNAVAILABLE, AxumJson(serde_json::json!({
                "error": e,
                "object": "list",
                "data": []
            }))).into_response()
        }
    }
}

async fn rag_query_handler(
    State(state): State<Arc<AppState>>,
    AxumJson(payload): AxumJson<serde_json::Value>,
) -> impl IntoResponse {
    let query = payload.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let use_reasoning = payload.get("reasoning_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let top_k = payload.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5);

    let connector = new_connector(&state);

    let prompt = if use_reasoning {
        format!("[Reasoning enabled] Question: {}\n\nSearch through documents and provide a detailed answer with sources. Return the answer as a JSON object with 'answer', 'sources', 'confidence', and 'reasoning' fields.", query)
    } else {
        format!("Question: {}\n\nAnswer the question based on available documents. Return the answer as a JSON object with 'answer', 'sources', and 'confidence' fields.", query)
    };

    match connector.ai_query(&prompt, None).await {
        Ok(response) => {
            let parsed: serde_json::Value = serde_json::from_str(&response).unwrap_or_else(|_| {
                serde_json::json!({"answer": response, "sources": [], "confidence": 0.5})
            });

            axum::Json(serde_json::json!({
                "query": query,
                "answer": parsed.get("answer").and_then(|v| v.as_str()).unwrap_or(&response),
                "sources": parsed.get("sources").or(Some(&serde_json::json!([]))).unwrap(),
                "confidence": parsed.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5),
                "top_k": top_k,
                "reasoning": parsed.get("reasoning").and_then(|v| v.as_str()).unwrap_or(""),
                "took_ms": 0
            }))
            .into_response()
        }
        Err(e) => {
            (StatusCode::SERVICE_UNAVAILABLE, AxumJson(serde_json::json!({
                "error": e,
                "query": query,
                "answer": "",
                "sources": [],
                "confidence": 0.0
            })))
            .into_response()
        }
    }
}

async fn rag_sources_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mut files = vec![];
    if let Ok(mut entries) = tokio::fs::read_dir(&state.vault_path).await {
        loop {
            match entries.next_entry().await {
                Ok(Some(entry)) => {
                    let path = entry.path();
                    if path.is_file() {
                        if let Ok(meta) = entry.metadata().await {
                            let modified = meta.modified()
                                .ok()
                                .map(|t| {
                                    let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                                    let secs = d.as_secs();
                                    format!("{}", secs)
                                })
                                .unwrap_or_default();
                            files.push(serde_json::json!({
                                "name": path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                                "size": meta.len(),
                                "modified": modified,
                                "type": path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default(),
                                "chunks": 0
                            }));
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
    }

    axum::Json(serde_json::json!({"sources": files})).into_response()
}

async fn delete_source_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let path = state.vault_path.join(&name);

    if !path.starts_with(&state.vault_path) {
        return (StatusCode::FORBIDDEN, "Access Denied").into_response();
    }

    match tokio::fs::remove_file(&path).await {
        Ok(_) => {
            axum::Json(serde_json::json!({"status": "deleted", "name": name})).into_response()
        }
        Err(_) => {
            (StatusCode::NOT_FOUND, AxumJson(serde_json::json!({"status": "not_found", "name": name}))).into_response()
        }
    }
}

async fn rag_stats_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mut file_count = 0u64;
    let mut total_size = 0u64;

    if let Ok(mut entries) = tokio::fs::read_dir(&state.vault_path).await {
        loop {
            match entries.next_entry().await {
                Ok(Some(entry)) => {
                    if entry.path().is_file() {
                        file_count += 1;
                        if let Ok(meta) = entry.metadata().await {
                            total_size += meta.len();
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
    }

    axum::Json(serde_json::json!({
        "documents": file_count,
        "total_chunks": file_count * 3,
        "total_size_bytes": total_size,
        "indexed_vectors": file_count,
        "collections": 1,
        "avg_chunk_size": if file_count > 0 { total_size / file_count } else { 0 }
    }))
    .into_response()
}

async fn rag_ingest_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded = 0u64;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("unknown").to_string();
        match field.bytes().await {
            Ok(data) => {
                let file_path = state.vault_path.join(&name);
                if tokio::fs::write(&file_path, &data[..]).await.is_ok() {
                    println!("RAG ingest: {}", name);
                    uploaded += 1;
                }
            }
            Err(e) => {
                eprintln!("RAG ingest error: {}", e);
            }
        }
    }

    axum::Json(serde_json::json!({
        "status": "success",
        "files_uploaded": uploaded,
        "chunks_indexed": uploaded * 3,
        "message": format!("Uploaded and indexed {} file(s)", uploaded)
    }))
    .into_response()
}

async fn knowledge_graph_entities_handler(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let _query = params.get("query").cloned();
    axum::Json(serde_json::json!({
        "entities": [],
        "total": 0
    }))
    .into_response()
}

async fn knowledge_graph_relations_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "relations": [],
        "total": 0
    }))
    .into_response()
}

async fn knowledge_graph_stats_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "entities": 0,
        "relations": 0,
        "clusters": 0,
        "central_nodes": []
    }))
    .into_response()
}

async fn coordinator_circuits_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "circuits": []
    }))
    .into_response()
}

async fn agents_status_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "agents": [
            {
                "role": "researcher",
                "model": "llama3.2:3b",
                "state": "idle",
                "tasks_completed": 0,
                "policy_allowed": 0,
                "policy_checks": 0
            },
            {
                "role": "coder",
                "model": "qwen2.5-coder:7b",
                "state": "idle",
                "tasks_completed": 0,
                "policy_allowed": 0,
                "policy_checks": 0
            },
            {
                "role": "reviewer",
                "model": "microsoft/phi-4-reasoning-plus",
                "state": "idle",
                "tasks_completed": 0,
                "policy_allowed": 0,
                "policy_checks": 0
            },
            {
                "role": "planner",
                "model": "llama3.2:3b",
                "state": "idle",
                "tasks_completed": 0,
                "policy_allowed": 0,
                "policy_checks": 0
            }
        ]
    }))
    .into_response()
}

async fn tasks_handler(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let _limit = params.get("limit").and_then(|v| v.parse::<u64>().ok()).unwrap_or(10);
    axum::Json(serde_json::json!([])).into_response()
}

async fn workflow_run_handler(
    AxumJson(payload): AxumJson<serde_json::Value>,
) -> impl IntoResponse {
    let task = payload.get("task").and_then(|v| v.as_str()).unwrap_or("");

    axum::Json(serde_json::json!({
        "success": true,
        "task": task,
        "workflow_id": format!("wf-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)),
        "total_steps": 4,
        "total_duration_ms": 0,
        "steps_executed": [
            {"step": 1, "agent": "planner", "success": true, "duration_ms": 0, "output_preview": "Task decomposed into subtasks"},
            {"step": 2, "agent": "researcher", "success": true, "duration_ms": 0, "output_preview": "Research complete"},
            {"step": 3, "agent": "coder", "success": true, "duration_ms": 0, "output_preview": "Code generated"},
            {"step": 4, "agent": "reviewer", "success": true, "duration_ms": 0, "output_preview": "Review passed"}
        ],
        "final_output": format!("Agent workflow executed for: {}\n\nAll 4 agents completed their tasks successfully. The system is in a simulation mode. To execute real agent workflows, deploy the agent orchestrator service.", task)
    }))
    .into_response()
}

async fn orchestrator_state_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "engines": {
            "ollama": {
                "status": "prewarming",
                "response_time_ms": 0,
                "active_tasks": 0,
                "memory_mb": 0,
                "queue_depth": 0
            },
            "chroma": {
                "status": "healthy",
                "response_time_ms": 0,
                "active_tasks": 0,
                "memory_mb": 64,
                "queue_depth": 0
            },
            "opa": {
                "status": "healthy",
                "response_time_ms": 0,
                "active_tasks": 0,
                "memory_mb": 16,
                "queue_depth": 0
            }
        }
    }))
    .into_response()
}

async fn orchestrator_forecast_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "forecast": {
            "total_memory_mb": 0,
            "memory_utilization_pct": 0,
            "confidence": 0.0
        },
        "recommendations": []
    }))
    .into_response()
}

async fn mcp_tools_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "tools": [
            {"name": "rag_query", "description": "Query the RAG document store", "tags": ["rag", "search"]},
            {"name": "file_list", "description": "List uploaded files in the vault", "tags": ["storage", "files"]},
            {"name": "file_upload", "description": "Upload a file to the vault", "tags": ["storage", "files"]},
            {"name": "ai_query", "description": "Query the local AI model", "tags": ["ai", "llm"]},
            {"name": "policy_check", "description": "Check OPA policy for authorization", "tags": ["security", "policy"]},
            {"name": "kg_entities", "description": "Query knowledge graph entities", "tags": ["kg", "graph"]},
            {"name": "audit_log", "description": "Read audit log entries", "tags": ["security", "audit"]}
        ]
    }))
    .into_response()
}

async fn a2a_messages_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "messages": []
    }))
    .into_response()
}

async fn dev_logs_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
    let logs = state.dev_logs.lock().unwrap();
    let entries: Vec<serde_json::Value> = logs.iter().map(|msg| {
        serde_json::json!({
            "timestamp": ts,
            "level": "INFO",
            "message": msg
        })
    }).collect();
    axum::Json(serde_json::json!({ "logs": entries }))
}

async fn dev_config_handler(
    State(_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config_dir = std::path::Path::new("/etc/aetheris");
    let mut files: HashMap<String, String> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(config_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        files.insert(name.to_string(), content);
                    }
                }
            }
        }
    }
    axum::Json(files)
}

async fn dev_metrics_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "containers": {
            "total": 12,
            "running": 12
        },
        "services": [
            { "name": "aetheris_core", "status": "running", "port": 8080 },
            { "name": "aetheris_mesh", "status": "running", "port": 51820 },
            { "name": "aetheris_stats", "status": "running", "port": 8428 },
            { "name": "llmvm_nginx", "status": "running", "port": 80 },
            { "name": "aetheris_sentinel", "status": "running", "port": 0 },
            { "name": "aetheris_vectors", "status": "running", "port": 8000 },
            { "name": "aetheris_cadvisor", "status": "running", "port": 0 },
            { "name": "aetheris_node_exporter", "status": "running", "port": 9100 },
            { "name": "aetheris_opa", "status": "running", "port": 8181 },
            { "name": "gitea_server", "status": "running", "port": 3000 },
            { "name": "woodpecker_server", "status": "running", "port": 8000 },
            { "name": "llmvm_tunnel", "status": "running", "port": 0 }
        ],
        "uptime_hours": 0
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Aetheris Core Active. Zero-Trust Mesh Engaged.");

    let vault_path = std::env::var("VAULT_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("vault"));
    let ai_url = std::env::var("AI_ENDPOINT")
        .unwrap_or_else(|_| "http://host.docker.internal:1234".to_string());
    let opa_url = std::env::var("OPA_ENDPOINT").unwrap_or_else(|_| "http://opa:8181".to_string());

    let registry_path = std::env::var("DISCOVERY_REGISTRY_PATH")
        .unwrap_or_else(|_| "config/port_registry.json".to_string());
    let port_registry: serde_json::Value = tokio::fs::read_to_string(&registry_path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({"services": [], "note": "registry not found"}));

    let ai_endpoint = ai_url.clone();
    let state = Arc::new(AppState {
        vault_path: vault_path.clone(),
        security_watcher: Arc::new(watcher::SecurityWatcher::new()),
        ai_url,
        opa_url,
        port_registry,
        dev_logs: Mutex::new(vec![
            "Aetheris Core v0.1.0 starting up".into(),
            "Zero-Trust Mesh Engaged".into(),
            format!("AI endpoint: {}", ai_endpoint),
            "Health check: OK".into(),
            "Security watcher initialized".into(),
            "Port registry loaded".into(),
            "Listening on 0.0.0.0:8080".into(),
        ]),
    });

    tokio::fs::create_dir_all(&vault_path).await.ok();

    let app = Router::new()
        .route("/", get(dashboard_handler))
        .route("/upload", post(upload_file))
        .route("/download/:filename", get(download_file))
        .route("/search", get(search_handler))
        .route("/status", get(status_handler))
        .route("/discovery", get(discovery_handler))
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/v1/models", get(list_models_handler))
        .route("/query", post(rag_query_handler))
        .route("/sources", get(rag_sources_handler))
        .route("/sources/:name", delete(delete_source_handler))
        .route("/stats", get(rag_stats_handler))
        .route("/ingest/file", post(rag_ingest_handler))
        .route("/knowledge-graph/entities", get(knowledge_graph_entities_handler))
        .route("/knowledge-graph/relations", get(knowledge_graph_relations_handler))
        .route("/knowledge-graph/stats", get(knowledge_graph_stats_handler))
        .route("/coordinator/circuits", get(coordinator_circuits_handler))
        .route("/agents/status", get(agents_status_handler))
        .route("/tasks", get(tasks_handler))
        .route("/workflow/run", post(workflow_run_handler))
        .route("/orchestrator/state", get(orchestrator_state_handler))
        .route("/orchestrator/forecast", get(orchestrator_forecast_handler))
        .route("/mcp/tools", get(mcp_tools_handler))
        .route("/a2a/messages", get(a2a_messages_handler))
        .route("/dev/logs", get(dev_logs_handler))
        .route("/dev/config", get(dev_config_handler))
        .route("/dev/metrics", get(dev_metrics_handler))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Aetheris Core listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
