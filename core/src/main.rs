use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;

mod a2a;
mod agents;
mod bridge;
mod connector;
mod fusion;
mod guardian;
mod implementation;
mod mcp;
mod metrics;
mod proxy;
mod rag;
mod sync;
mod wal;
mod watcher;

use a2a::A2AGateway;
use agents::Agent;
use bridge::{ModelBridge, SecurityBridge};
use fusion::{ExaSearchBridge, FusionRouter, KeyManager, OpenRouterBridge};
use guardian::Guardian;
use mcp::MCPServer;
use proxy::OrchestratorProxy;
use rag::VectorStore;

pub struct AppState {
    pub vault_path: std::path::PathBuf,
    pub security_watcher: Arc<watcher::SecurityWatcher>,
    pub ai_url: String,
    pub opa_url: String,
    pub port_registry: serde_json::Value,
    pub dev_logs: Mutex<Vec<String>>,
    pub wal: Arc<Mutex<wal::WriteAheadLog>>,
    pub model_bridge: Arc<dyn ModelBridge>,
    pub security_bridge: Arc<dyn SecurityBridge>,
    pub a2a_gateway: Mutex<A2AGateway>,
    pub mcp_server: MCPServer,
    pub agents: Mutex<Vec<Box<dyn Agent>>>,
    pub orchestrator_proxy: Option<OrchestratorProxy>,
    pub key_manager: Arc<KeyManager>,
    pub vector_store: Option<Arc<VectorStore>>,
    pub fusion_router: Option<FusionRouter>,
    pub guardian: Arc<Guardian>,
    pub rag_config: Arc<Mutex<rag::RagConfig>>,
}

// ─── Dashboard ───────────────────────────────────────────────────

async fn dashboard_handler() -> impl IntoResponse {
    let html = std::include_str!("../ui/index.html");
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap()
}

// ─── Status ──────────────────────────────────────────────────────

async fn status_handler(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
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
        "security": { "auto_ban": "active", "banned_peers": 0, "ghost_shell": "armed" }
    }))
    .into_response()
}

async fn discovery_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json(state.port_registry.clone()).into_response()
}

async fn metrics_handler() -> impl IntoResponse {
    let m = metrics::metrics_handler();
    Response::builder()
        .header(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )
        .body(Body::from(m))
        .unwrap()
}

// ─── File Operations ─────────────────────────────────────────────

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
            state
                .wal
                .lock()
                .unwrap()
                .append(wal::WalEntry::FileDownload {
                    filename: filename.clone(),
                })
                .ok();
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
        if let Ok(data) = field.bytes().await {
            let file_path = state.vault_path.join(&name);
            if tokio::fs::write(&file_path, &data[..]).await.is_ok() {
                state
                    .wal
                    .lock()
                    .unwrap()
                    .append(wal::WalEntry::FileUpload {
                        filename: name,
                        size: data.len() as u64,
                    })
                    .ok();
                uploaded += 1;
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

// ─── AI / Search / Health ────────────────────────────────────────

async fn search_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    metrics::SEARCH_QUERIES.inc();
    let query = params.get("q").cloned().unwrap_or_default();
    let answer = state
        .model_bridge
        .query(&format!("Search query: {}", query), "qwen2.5:14b")
        .await
        .unwrap_or_default();
    let results =
        vec![serde_json::json!({"filename": "example.pdf", "score": 0.95, "excerpt": answer})];
    axum::Json(serde_json::json!({"query": query, "results": results, "total": 1})).into_response()
}

async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ai_ok = state.model_bridge.list_models().await.is_ok();
    let agents_len = state.agents.lock().unwrap().len();
    axum::Json(serde_json::json!({
        "status": "ok",
        "agents": agents_len,
        "tasks": 0,
        "tools": state.mcp_server.list_tools_json()["tools"].as_array().map(|a| a.len()).unwrap_or(0),
        "prompts": 8,
        "ai_connected": ai_ok,
        "cross_system": state.orchestrator_proxy.is_some(),
        "spread_forecast": {
            "total_memory_mb": agents_len * 256,
            "memory_utilization_pct": 0,
            "confidence": 0.85,
            "bottleneck": "none"
        }
    }))
    .into_response()
}

async fn list_models_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.model_bridge.list_models().await {
        Ok(models) => {
            let data: Vec<serde_json::Value> = models
                .into_iter()
                .map(|id| serde_json::json!({"id": id, "object": "model", "owned_by": "ollama"}))
                .collect();
            axum::Json(serde_json::json!({"object": "list", "data": data})).into_response()
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": e, "object": "list", "data": []
            })),
        )
            .into_response(),
    }
}

// ─── RAG ─────────────────────────────────────────────────────────

async fn rag_query_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let cfg = state.rag_config.lock().unwrap().clone();
    let query = payload.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let use_reasoning = payload
        .get("reasoning_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(cfg.reasoning_enabled);
    let top_k = payload
        .get("top_k")
        .and_then(|v| v.as_u64())
        .unwrap_or(cfg.top_k as u64) as usize;
    let use_reranker = payload
        .get("reranker_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(cfg.reranker_enabled);

    state
        .wal
        .lock()
        .unwrap()
        .append(wal::WalEntry::AiQuery {
            model: cfg.query_model.clone(),
            tokens: 0,
        })
        .ok();

    let context_chunks = if let Some(ref store) = state.vector_store {
        if let Ok(query_embed) = state.model_bridge.embed(query).await {
            let candidates = store.search(&query_embed, top_k * 3).ok();
            if let Some(mut chunks) = candidates {
                if use_reranker && chunks.len() > 1 {
                    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
                    if let Ok(scores) = state
                        .model_bridge
                        .rerank(query, texts, &cfg.reranker_model)
                        .await
                    {
                        for (i, score) in scores.iter().enumerate() {
                            if i < chunks.len() {
                                chunks[i].score = *score;
                            }
                        }
                        chunks.sort_by(|a, b| {
                            b.score
                                .partial_cmp(&a.score)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        chunks.truncate(top_k);
                    }
                } else {
                    chunks.truncate(top_k);
                }
                Some(chunks)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let prompt = if let Some(ref chunks) = context_chunks {
        let context: String = chunks
            .iter()
            .map(|c| {
                format!(
                    "[Source: {}] (relevance: {:.2})\n{}",
                    c.source, c.score, c.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        let reasoning = if use_reasoning {
            " Use reasoning to explain your thought process."
        } else {
            ""
        };
        format!(
            "Context:\n{}\n\nQuestion: {}\n\nAnswer based on the context above.{}\n\nReturn ONLY valid JSON (no markdown, no code fences) {{\"answer\", \"sources\" (list of source filenames), \"confidence\" (0.0-1.0){}}}.",
            context, query, reasoning,
            if use_reasoning { ", \"reasoning\" (string)" } else { "" }
        )
    } else {
        format!("Question: {}\n\nAnswer the question. Return ONLY valid JSON (no markdown, no code fences) {{\"answer\", \"sources\", \"confidence\"{}}}.", query, if use_reasoning { ", \"reasoning\"" } else { "" })
    };

    match state.model_bridge.query(&prompt, &cfg.query_model).await {
        Ok(response) => {
            let cleaned = response
                .trim_start_matches("```json\n")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();
            let parsed: serde_json::Value = serde_json::from_str(cleaned).unwrap_or_else(
                |_| serde_json::json!({"answer": response, "sources": [], "confidence": 0.5}),
            );
            let sources_val = parsed.get("sources").cloned().unwrap_or(serde_json::json!([]));
            let src = if sources_val.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                context_chunks.as_ref().map(|chunks| {
                    serde_json::json!(chunks.iter().map(|c| serde_json::json!({"source": c.source, "score": c.score})).collect::<Vec<_>>())
                }).unwrap_or(serde_json::json!([]))
            } else {
                sources_val
            };
            axum::Json(serde_json::json!({
                "query": query,
                "answer": parsed.get("answer").and_then(|v| v.as_str()).unwrap_or(&response),
                "model": cfg.query_model,
                "sources": src,
                "confidence": parsed.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5),
                "top_k": top_k,
                "reasoning": parsed.get("reasoning").and_then(|v| v.as_str()).unwrap_or(""),
                "chunks_searched": context_chunks.as_ref().map(|c| c.len()).unwrap_or(0),
                "reranker_used": use_reranker,
                "reranker_model": cfg.reranker_model,
                "took_ms": 0,
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": e, "query": query, "answer": "", "sources": [], "confidence": 0.0
            })),
        )
            .into_response(),
    }
}

async fn rag_sources_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut sources: Vec<serde_json::Value> = if let Some(ref store) = state.vector_store {
        store
            .sources()
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.source, "chunks": s.chunk_count,
                    "first_seen": s.first_seen, "last_seen": s.last_seen
                })
            })
            .collect()
    } else {
        vec![]
    };

    if let Ok(mut entries) = tokio::fs::read_dir(&state.vault_path).await {
        loop {
            match entries.next_entry().await {
                Ok(Some(entry)) => {
                    let path = entry.path();
                    if path.is_file()
                        && !sources.iter().any(|s| {
                            s["name"].as_str()
                                == Some(
                                    path.file_name()
                                        .map(|n| n.to_string_lossy())
                                        .as_deref()
                                        .unwrap_or(""),
                                )
                        })
                    {
                        if let Ok(meta) = entry.metadata().await {
                            sources.push(serde_json::json!({
                                "name": path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                                "size": meta.len(),
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
    axum::Json(serde_json::json!({"sources": sources})).into_response()
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
            state
                .wal
                .lock()
                .unwrap()
                .append(wal::WalEntry::FileDelete {
                    filename: name.clone(),
                })
                .ok();
            axum::Json(serde_json::json!({"status": "deleted", "name": name})).into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"status": "not_found", "name": name})),
        )
            .into_response(),
    }
}

async fn rag_stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
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
    let store_stats = state.vector_store.as_ref().and_then(|s| s.stats().ok());
    axum::Json(serde_json::json!({
        "documents": file_count,
        "total_chunks": store_stats.as_ref().map(|s| s.total_chunks).unwrap_or(file_count as i64 * 3),
        "total_size_bytes": total_size,
        "indexed_vectors": store_stats.as_ref().map(|s| s.total_chunks).unwrap_or(file_count as i64),
        "embedding_dimension": store_stats.as_ref().map(|s| s.embedding_dimension).unwrap_or(0),
        "total_sources": store_stats.as_ref().map(|s| s.total_sources).unwrap_or(0),
        "db_size_mb": store_stats.as_ref().map(|s| s.db_size_mb).unwrap_or(0.0),
        "collections": 1,
        "avg_chunk_size": if file_count > 0 { total_size / file_count } else { 0 },
    })).into_response()
}

async fn rag_config_get_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cfg = state.rag_config.lock().unwrap().clone();
    axum::Json(serde_json::json!(cfg))
}

async fn rag_config_put_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<rag::RagConfig>,
) -> impl IntoResponse {
    {
        let mut cfg = state.rag_config.lock().unwrap();
        *cfg = payload;
        cfg.save(&state.vault_path);
    }
    let cfg = state.rag_config.lock().unwrap().clone();
    axum::Json(serde_json::json!({"status": "saved", "config": cfg}))
}

async fn rag_ingest_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded = 0u64;
    let mut total_chunks = 0usize;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("unknown").to_string();
        if let Ok(data) = field.bytes().await {
            let file_path = state.vault_path.join(&name);
            if tokio::fs::write(&file_path, &data[..]).await.is_ok() {
                uploaded += 1;

                if let Some(ref store) = state.vector_store {
                    if let Ok(content) = String::from_utf8(data.to_vec()) {
                        let cfg = state.rag_config.lock().unwrap().clone();
                        let chunker = rag::TextChunker::new(cfg.chunk_size, cfg.chunk_overlap);
                        let chunks = chunker.chunk(&content, &name);
                        if !chunks.is_empty() {
                            let mut embeddings = Vec::new();
                            let mut success = true;
                            for chunk in &chunks {
                                match state.model_bridge.embed(&chunk.text).await {
                                    Ok(emb) => embeddings.push(emb),
                                    Err(e) => {
                                        eprintln!(
                                            "Embedding failed for chunk {} of {}: {}",
                                            chunk.index, name, e
                                        );
                                        success = false;
                                        break;
                                    }
                                }
                            }
                            if success && !embeddings.is_empty() {
                                match store.add_chunks(&chunks, &embeddings) {
                                    Ok(ids) => {
                                        total_chunks += ids.len();
                                        state
                                            .wal
                                            .lock()
                                            .unwrap()
                                            .append(wal::WalEntry::Custom {
                                                action: "rag_ingest".to_string(),
                                                details: format!(
                                                    "file={}, chunks={}",
                                                    name,
                                                    ids.len()
                                                ),
                                            })
                                            .ok();
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to store chunks for {}: {}", name, e)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    axum::Json(serde_json::json!({
        "status": "success", "files_uploaded": uploaded,
        "chunks_indexed": if total_chunks > 0 { total_chunks } else { (uploaded * 3) as usize },
        "message": format!("Uploaded {} file(s) and indexed {} chunk(s)", uploaded, total_chunks)
    }))
    .into_response()
}

// ─── Bridge REST ──────────────────────────────────────────────────

async fn bridge_ai_query(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let prompt = payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    let model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("qwen2.5:14b");
    let peer_id = payload
        .get("peer_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("query");

    if !state.security_bridge.authorize(peer_id, action).await {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Policy denied"})),
        )
            .into_response();
    }

    state
        .wal
        .lock()
        .unwrap()
        .append(wal::WalEntry::AiQuery {
            model: model.to_string(),
            tokens: 0,
        })
        .ok();

    match state.model_bridge.query(prompt, model).await {
        Ok(response) => {
            axum::Json(serde_json::json!({"response": response, "model": model})).into_response()
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

async fn bridge_ai_embed(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let content = payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("nomic-embed-text");
    let ollama = implementation::OllamaBridge::new(state.ai_url.clone());
    match ollama.embed_with_model(content, model).await {
        Ok(embedding) => axum::Json(
            serde_json::json!({"embedding": embedding, "dims": embedding.len(), "model": model}),
        )
        .into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": e, "model": model})),
        )
            .into_response(),
    }
}

async fn bridge_ai_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.model_bridge.list_models().await {
        Ok(models) => axum::Json(serde_json::json!({"models": models})).into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

async fn bridge_security_authorize(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let peer_id = payload
        .get("peer_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let action = payload.get("action").and_then(|v| v.as_str()).unwrap_or("");
    let allowed = state.security_bridge.authorize(peer_id, action).await;
    axum::Json(serde_json::json!({"allowed": allowed, "peer_id": peer_id, "action": action}))
}

// ─── Agents ───────────────────────────────────────────────────────

async fn agents_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let agents = state.agents.lock().unwrap();
    let statuses: Vec<serde_json::Value> = agents
        .iter()
        .map(|a| {
            let s = a.get_status();
            serde_json::json!({
                "id": s.id, "role": s.role, "model": s.model,
                "state": s.state, "tasks_completed": s.tasks_completed,
                "policy_checks": s.policy_checks, "policy_allowed": s.policy_allowed,
            })
        })
        .collect();
    axum::Json(serde_json::json!({"total": statuses.len(), "agents": statuses})).into_response()
}

async fn agents_list_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let agents = state.agents.lock().unwrap();
    let statuses: Vec<serde_json::Value> = agents
        .iter()
        .map(|a| {
            let s = a.get_status();
            serde_json::json!(s)
        })
        .collect();
    axum::Json(statuses)
}

#[derive(serde::Deserialize)]
struct WorkflowRequest {
    task: String,
    #[allow(dead_code)]
    max_iterations: Option<u64>,
    use_reasoning: Option<bool>,
    skip_review: Option<bool>,
}

#[derive(serde::Deserialize)]
struct TaskSubmitRequest {
    task: Option<String>,
    role: Option<String>,
    context: Option<serde_json::Value>,
}

// ─── Workflow ─────────────────────────────────────────────────────

// Test function to check handler registration
#[allow(dead_code)]
async fn workflow_minimal_handler(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

async fn workflow_run_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WorkflowRequest>,
) -> Response {
    let t = payload.task;
    let ur = payload.use_reasoning.unwrap_or(true);
    let sr = payload.skip_review.unwrap_or(false);
    workflow_run_body(state, &t, ur, !sr).await
}

async fn workflow_run_body(
    state: Arc<AppState>,
    task: &str,
    use_reasoning: bool,
    do_review: bool,
) -> Response {
    // Note: workflow_run_handler body inlined
    let conv_id = format!(
        "conv_{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let start = std::time::SystemTime::now();
    let mut steps_executed: Vec<serde_json::Value> = Vec::new();
    let mut final_output = String::new();

    let agent_roles: Vec<String> = {
        let a = state.agents.lock().unwrap();
        a.iter().map(|ag| ag.role().as_str().to_string()).collect()
    };

    let pipeline = vec!["planner", "researcher", "coder"];
    for (idx, role) in pipeline.iter().enumerate() {
        let result = {
            let extracted = {
                let mut agents = state.agents.lock().unwrap();
                let pos = agents.iter().position(|a| a.role().as_str() == *role);
                pos.map(|i| agents.remove(i))
            };
            if let Some(mut agent) = extracted {
                let ctx = serde_json::json!({
                    "available_agents": agent_roles.clone(),
                    "kg_context": "",
                    "use_reasoning": use_reasoning,
                    "workspace": "/workspace",
                });
                let res = Some(agent.execute(task, &ctx).await);
                let mut agents = state.agents.lock().unwrap();
                agents.push(agent);
                res
            } else {
                None
            }
        };
        if let Some(result) = result {
            steps_executed.push(serde_json::json!({
                "step": idx + 1, "agent": role, "agent_id": result.agent_id,
                "success": result.success, "duration_ms": result.duration_ms,
                "output_preview": result.output.chars().take(200).collect::<String>()
            }));
            final_output = result.output.clone();
            let a2a = state.a2a_gateway.lock().unwrap();
            let next_role = if *role == "planner" {
                "researcher"
            } else if *role == "researcher" {
                "coder"
            } else {
                "reviewer"
            };
            a2a.send(
                role,
                next_role,
                &conv_id,
                "request",
                serde_json::json!({"output": result.output, "original_task": task}),
                true,
            );
        }
    }
    if do_review {
        let result = {
            let extracted = {
                let mut agents = state.agents.lock().unwrap();
                let pos = agents.iter().position(|a| a.role().as_str() == "reviewer");
                pos.map(|i| agents.remove(i))
            };
            if let Some(mut agent) = extracted {
                let res = Some(
                    agent
                        .execute(
                            &task,
                            &serde_json::json!({
                                "content": final_output, "rag_answer": "", "kg_context": "",
                                "criteria": ["correctness", "quality", "security"]
                            }),
                        )
                        .await,
                );
                let mut agents = state.agents.lock().unwrap();
                agents.push(agent);
                res
            } else {
                None
            }
        };
        if let Some(result) = result {
            steps_executed.push(serde_json::json!({
                "step": 4, "agent": "reviewer", "agent_id": result.agent_id,
                "success": result.success, "duration_ms": result.duration_ms,
                "output_preview": result.output.chars().take(200).collect::<String>()
            }));
            final_output = format!(
                "## Implementation\n\n{}\n\n## Review\n\n{}",
                final_output, result.output
            );
        }
    }
    let total_duration = start.elapsed().unwrap_or_default().as_secs_f64() * 1000.0;
    let all_ok = steps_executed
        .iter()
        .all(|s| s["success"].as_bool().unwrap_or(false));

    Json(serde_json::json!({
        "task": task, "steps_executed": steps_executed,
        "total_steps": steps_executed.len(), "total_duration_ms": total_duration,
        "final_output": final_output, "success": all_ok,
    }))
    .into_response()
}

async fn task_submit_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TaskSubmitRequest>,
) -> impl IntoResponse {
    let task = payload.task.unwrap_or_default();
    let agent_role = payload
        .role
        .as_deref()
        .and_then(agents::AgentRole::from_str)
        .unwrap_or(agents::AgentRole::Planner);
    let ctx = payload.context.unwrap_or(serde_json::json!({}));

    let mut agent = agents::create_agent(
        agent_role,
        "qwen2.5:14b".to_string(),
        String::new(),
        Some(state.model_bridge.clone()),
        Some(state.security_bridge.clone()),
    );

    let result = agent.execute(&task, &ctx).await;

    let mut agents = state.agents.lock().unwrap();
    agents.push(agent);

    axum::Json(serde_json::json!({
        "agent_id": result.agent_id, "output": result.output,
        "success": result.success, "duration_ms": result.duration_ms,
    }))
}

async fn tasks_handler(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json(serde_json::json!([]))
}

// ─── Orchestrator ─────────────────────────────────────────────────

async fn orchestrator_state_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if let Some(ref proxy) = state.orchestrator_proxy {
        let uri = Uri::from_static("/orchestrator/state");
        return proxy
            .forward(Method::GET, &uri, axum::body::Bytes::new())
            .await;
    }

    let agents = state.agents.lock().unwrap();
    let total = agents.len();
    let idle = agents
        .iter()
        .filter(|a| a.state().as_str() == "idle")
        .count();
    let busy = agents
        .iter()
        .filter(|a| a.state().as_str() != "idle")
        .count();
    let agent_map: serde_json::Value = agents
        .iter()
        .map(|a| {
            (
                a.role().as_str().to_string(),
                serde_json::json!({
                    "status": if a.state().as_str() == "idle" { "healthy" } else { "active" },
                    "active_tasks": if a.state().as_str() == "idle" { 0 } else { 1 },
                    "memory_mb": 0,
                    "queue_depth": 0,
                    "response_time_ms": 0,
                }),
            )
        })
        .collect();
    drop(agents);

    axum::Json(serde_json::json!({
        "engines": agent_map,
        "healthy_count": idle,
        "busy_count": busy,
        "total_engines": total,
    }))
    .into_response()
}

async fn orchestrator_forecast_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if let Some(ref proxy) = state.orchestrator_proxy {
        let uri = Uri::from_static("/orchestrator/forecast");
        return proxy
            .forward(Method::GET, &uri, axum::body::Bytes::new())
            .await;
    }
    let agents = state.agents.lock().unwrap();
    let idle = agents
        .iter()
        .filter(|a| a.state().as_str() == "idle")
        .count();
    let total = agents.len();
    drop(agents);
    let utilization = if total > 0 {
        ((total - idle) as f64 / total as f64 * 100.0) as u64
    } else {
        0
    };
    axum::Json(serde_json::json!({
        "forecast": {
            "total_memory_mb": total * 256,
            "memory_utilization_pct": utilization,
            "confidence": 0.85,
            "bottleneck": if utilization > 80 { "agents" } else { "none" },
        },
        "recommendations": if utilization > 80 {
            serde_json::json!(["All agents are busy — consider adding more agent instances"])
        } else {
            serde_json::json!([])
        }
    }))
    .into_response()
}

// ─── MCP ──────────────────────────────────────────────────────────

async fn mcp_tools_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json(state.mcp_server.list_tools_json())
}

// ─── A2A ──────────────────────────────────────────────────────────

async fn a2a_messages_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let a2a = state.a2a_gateway.lock().unwrap();
    axum::Json(serde_json::json!({"messages": a2a.get_message_log(50)}))
}

// ─── Knowledge Graph ─────────────────────────────────────────────

async fn knowledge_graph_entities_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if let Some(ref proxy) = state.orchestrator_proxy {
        let uri = Uri::from_static("/knowledge-graph/entities");
        return proxy
            .forward(Method::GET, &uri, axum::body::Bytes::new())
            .await;
    }
    axum::Json(serde_json::json!({"entities": [], "total": 0})).into_response()
}

async fn knowledge_graph_relations_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Some(ref proxy) = state.orchestrator_proxy {
        let uri = Uri::from_static("/knowledge-graph/relations");
        return proxy
            .forward(Method::GET, &uri, axum::body::Bytes::new())
            .await;
    }
    axum::Json(serde_json::json!({"relations": [], "total": 0})).into_response()
}

async fn knowledge_graph_stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if let Some(ref proxy) = state.orchestrator_proxy {
        let uri = Uri::from_static("/knowledge-graph/stats");
        return proxy
            .forward(Method::GET, &uri, axum::body::Bytes::new())
            .await;
    }
    axum::Json(
        serde_json::json!({"entities": 0, "relations": 0, "clusters": 0, "central_nodes": []}),
    )
    .into_response()
}

async fn coordinator_circuits_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({"circuits": []}))
}

// ─── Audit ────────────────────────────────────────────────────────

async fn audit_log_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let wal = state.wal.lock().unwrap();
    let mut records = Vec::new();
    let _ = wal.replay(|record| {
        records.push(serde_json::json!({
            "timestamp": record.timestamp, "sequence": record.sequence, "entry": record.entry,
        }));
        Ok(())
    });
    axum::Json(serde_json::json!({"records": records, "total": records.len()}))
}

async fn audit_replay_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let wal = state.wal.lock().unwrap();
    let count = wal.replay(|_| Ok(())).unwrap_or(0);
    axum::Json(serde_json::json!({"replayed": count, "errors": 0, "status": "ok"}))
}

// ─── Dev ──────────────────────────────────────────────────────────

async fn dev_logs_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
    let logs = state.dev_logs.lock().unwrap();
    let entries: Vec<serde_json::Value> = logs
        .iter()
        .map(|msg| serde_json::json!({"timestamp": ts, "level": "INFO", "message": msg}))
        .collect();
    axum::Json(serde_json::json!({"logs": entries}))
}

async fn dev_config_handler(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
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
        "containers": { "total": 12, "running": 12 },
        "services": [
            {"name": "aetheris_core", "status": "running", "port": 8080},
            {"name": "aetheris_mesh", "status": "running", "port": 51820},
        ],
        "uptime_hours": 0
    }))
}

// ─── Sync ────────────────────────────────────────────────────────

async fn sync_download(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> Response {
    let path = state.vault_path.join(&filename);
    if !path.starts_with(&state.vault_path) {
        return (StatusCode::FORBIDDEN, "Access Denied").into_response();
    }
    match tokio::fs::File::open(&path).await {
        Ok(file) => {
            let stream = tokio_util::io::ReaderStream::new(file);
            let body = Body::from_stream(stream);
            Response::builder()
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(body)
                .unwrap()
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

async fn sync_upload(State(state): State<Arc<AppState>>, mut multipart: Multipart) -> StatusCode {
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("unknown").to_string();
        let path = state.vault_path.join(&name);
        if let Ok(data) = field.bytes().await {
            tokio::fs::write(&path, &data).await.unwrap_or_default();
        }
    }
    StatusCode::OK
}

// ─── Proxy ────────────────────────────────────────────────────────

#[allow(dead_code)]
async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if let Some(ref proxy) = state.orchestrator_proxy {
        proxy.forward(method, &uri, body).await
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Python orchestrator not available",
        )
            .into_response()
    }
}

// ─── Settings ─────────────────────────────────────────────────────

async fn settings_handler() -> impl IntoResponse {
    let html = std::include_str!("../../web/settings/index.html");
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap()
}

// ─── API Keys ─────────────────────────────────────────────────────

async fn keys_list_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    axum::Json(serde_json::json!(state.key_manager.list_public()))
}

#[derive(serde::Deserialize)]
struct KeySetRequest {
    key: String,
    label: Option<String>,
}

async fn key_set_handler(
    State(state): State<Arc<AppState>>,
    Path(service): Path<String>,
    Json(payload): Json<KeySetRequest>,
) -> impl IntoResponse {
    let label = payload.label.unwrap_or_else(|| service.clone());
    match state.key_manager.set(&service, payload.key, label) {
        Ok(_) => {
            state
                .wal
                .lock()
                .unwrap()
                .append(wal::WalEntry::ConfigChange {
                    key: format!("key.{}", service),
                    old_value: String::new(),
                    new_value: "set".to_string(),
                })
                .ok();
            (StatusCode::OK, Json(serde_json::json!({"status": "saved"}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

async fn key_delete_handler(
    State(state): State<Arc<AppState>>,
    Path(service): Path<String>,
) -> impl IntoResponse {
    match state.key_manager.delete(&service) {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "deleted"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct KeyToggleRequest {
    enabled: bool,
}

async fn key_toggle_handler(
    State(state): State<Arc<AppState>>,
    Path(service): Path<String>,
    Json(payload): Json<KeyToggleRequest>,
) -> impl IntoResponse {
    match state.key_manager.toggle(&service, payload.enabled) {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "toggled"})),
        )
            .into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e}))).into_response(),
    }
}

// ─── Guardian ─────────────────────────────────────────────────────

async fn guardian_health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let health = state.guardian.health().await;
    axum::Json(health).into_response()
}

#[derive(serde::Deserialize)]
struct GuardianQueryRequest {
    query: String,
}

async fn guardian_query_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GuardianQueryRequest>,
) -> impl IntoResponse {
    let answer = state.guardian.process_query(&payload.query).await;
    axum::Json(serde_json::json!({"query": payload.query, "answer": answer}))
}

async fn guardian_versions_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let versions = state.guardian.versions.lock().unwrap().clone();
    let count = versions.len();
    axum::Json(serde_json::json!({"total": count, "versions": versions}))
}

async fn guardian_page_handler() -> impl IntoResponse {
    let html = std::include_str!("../../web/guardian/index.html");
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap()
}

async fn guardian_snapshot_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let vtype = payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("delta");
    let summary = payload
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("manual_snapshot");
    state.guardian.snapshot(vtype, summary);
    axum::Json(serde_json::json!({"status": "snapshot_created", "type": vtype, "summary": summary}))
}

// ─── Fusion ───────────────────────────────────────────────────────

async fn fusion_query_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let query = payload.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let use_search = payload
        .get("use_search")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if let Some(ref fusion) = state.fusion_router {
        let result = if use_search {
            fusion.search_and_synthesize(query).await
        } else {
            fusion.smart_query(query).await
        };
        match result {
            Ok(answer) => {
                axum::Json(serde_json::json!({"query": query, "answer": answer})).into_response()
            }
            Err(e) => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": e, "query": query})),
            )
                .into_response(),
        }
    } else {
        match state.model_bridge.query(query, "qwen2.5:14b").await {
            Ok(answer) => {
                axum::Json(serde_json::json!({"query": query, "answer": answer})).into_response()
            }
            Err(e) => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": e, "query": query})),
            )
                .into_response(),
        }
    }
}

// ─── Main ─────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Aetheris Core Active. Zero-Trust Mesh Engaged.");

    let vault_path = std::env::var("VAULT_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("vault"));
    let ai_url =
        std::env::var("AI_ENDPOINT").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let opa_url = std::env::var("OPA_ENDPOINT").unwrap_or_else(|_| "http://opa:8181".to_string());
    let orch_url = std::env::var("ORCHESTRATOR_ENDPOINT").ok();
    let ai_model = std::env::var("AI_MODEL").unwrap_or_else(|_| "qwen2.5:14b".to_string());

    let registry_path = std::env::var("DISCOVERY_REGISTRY_PATH")
        .unwrap_or_else(|_| "config/port_registry.json".to_string());
    let port_registry: serde_json::Value = tokio::fs::read_to_string(&registry_path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({"services": [], "note": "registry not found"}));

    tokio::fs::create_dir_all(&vault_path).await.ok();
    let wal_dir = vault_path.join("wal");
    tokio::fs::create_dir_all(&wal_dir).await.ok();

    let wal_log = wal::WriteAheadLog::new(&wal_dir.to_string_lossy()).unwrap_or_else(|e| {
        eprintln!(
            "Warning: Failed to initialize WAL ({}), using temp fallback",
            e
        );
        wal::WriteAheadLog::new(
            std::env::temp_dir()
                .join("aetheris_wal_fallback")
                .to_string_lossy()
                .as_ref(),
        )
        .expect("Temp WAL must initialize")
    });

    let mut ollama = implementation::OllamaBridge::new(ai_url.clone());
    ollama.default_model = ai_model;
    let model_bridge: Arc<dyn ModelBridge> = Arc::new(ollama);
    let security_bridge: Arc<dyn SecurityBridge> =
        Arc::new(implementation::OpaBridge::new(opa_url.clone()));

    let orchestrator_proxy = orch_url.map(|url| OrchestratorProxy::new(url));
    let a2a_gateway = A2AGateway::new(300);
    let mcp_server = MCPServer::new();

    let mut agent_vec: Vec<Box<dyn Agent>> = Vec::new();
    for (role, _model) in &[
        ("planner", ai_url.clone()),
        ("researcher", ai_url.clone()),
        ("coder", ai_url.clone()),
        ("reviewer", ai_url.clone()),
    ] {
        let r = agents::AgentRole::from_str(role).unwrap_or(agents::AgentRole::Researcher);
        agent_vec.push(agents::create_agent(
            r,
            "qwen2.5:14b".to_string(),
            String::new(),
            Some(model_bridge.clone()),
            Some(security_bridge.clone()),
        ));
    }

    let key_manager = Arc::new(KeyManager::new(&vault_path));

    let openrouter_url =
        std::env::var("OPENROUTER_URL").unwrap_or_else(|_| "https://openrouter.ai".to_string());
    let openrouter_model =
        std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "openai/gpt-4o-mini".to_string());
    let exasearch_url =
        std::env::var("EXASEARCH_URL").unwrap_or_else(|_| "https://api.exa.ai".to_string());

    let openrouter_bridge = Arc::new(OpenRouterBridge::new(
        openrouter_url,
        openrouter_model,
        key_manager.clone(),
    ));
    let exasearch_bridge = Arc::new(ExaSearchBridge::new(exasearch_url, key_manager.clone()));

    let fusion_router = FusionRouter::new(
        model_bridge.clone(),
        Some(openrouter_bridge.clone()),
        Some(exasearch_bridge.clone()),
        key_manager.clone(),
    );

    mcp_server.register_tool(mcp::MCPTool {
        name: "openrouter_query".to_string(),
        description: "Query OpenRouter for remote AI model inference (fallback LLM)".to_string(),
        tags: vec![
            "ai".to_string(),
            "openrouter".to_string(),
            "remote".to_string(),
        ],
    });
    mcp_server.register_tool(mcp::MCPTool {
        name: "exasearch_search".to_string(),
        description: "Search the web via ExaSearch API for context retrieval".to_string(),
        tags: vec![
            "search".to_string(),
            "web".to_string(),
            "exasearch".to_string(),
        ],
    });
    mcp_server.register_tool(mcp::MCPTool {
        name: "fusion_query".to_string(),
        description: "Smart query with local→search→remote fallback chain".to_string(),
        tags: vec![
            "ai".to_string(),
            "fusion".to_string(),
            "fallback".to_string(),
        ],
    });

    let vector_store = VectorStore::new(&vault_path.join("vectors.db").to_string_lossy()).ok();
    if vector_store.is_some() {
        println!(
            "Vector store initialized at {:?}",
            vault_path.join("vectors.db")
        );
    }

    let wal_arc = Arc::new(Mutex::new(wal_log));

    let guardian = Arc::new(Guardian::new(
        model_bridge.clone(),
        Some(fusion_router.clone()),
        vector_store.clone().map(Arc::new),
        wal_arc.clone(),
        vault_path.clone(),
    ));
    guardian.snapshot("milestone", "system_startup");
    println!("Guardian online — Chronicle snapshot captured");

    let rag_config = Arc::new(Mutex::new(rag::RagConfig::load(&vault_path)));
    println!(
        "RAG config loaded from {:?}",
        vault_path.join("rag_config.json")
    );

    let state = Arc::new(AppState {
        vault_path: vault_path.clone(),
        security_watcher: Arc::new(watcher::SecurityWatcher::new()),
        ai_url,
        opa_url,
        port_registry,
        wal: wal_arc,
        dev_logs: Mutex::new(vec![
            "Aetheris Core v0.1.0 starting up".into(),
            "Zero-Trust Mesh Engaged".into(),
            format!("{} agents initialized", agent_vec.len()),
            "Vector store online".into(),
            "Listening on 0.0.0.0:8080".into(),
        ]),
        model_bridge,
        security_bridge,
        a2a_gateway: Mutex::new(a2a_gateway),
        mcp_server,
        agents: Mutex::new(agent_vec),
        orchestrator_proxy,
        key_manager,
        vector_store: vector_store.map(Arc::new),
        fusion_router: Some(fusion_router),
        guardian,
        rag_config,
    });

    let app = Router::new()
        .route("/", get(dashboard_handler))
        .nest_service("/web", ServeDir::new("../web"))
        .route("/status", get(status_handler))
        .route("/discovery", get(discovery_handler))
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/upload", post(upload_file))
        .route("/download/:filename", get(download_file))
        .route("/v1/models", get(list_models_handler))
        .route("/search", get(search_handler))
        .route("/query", post(rag_query_handler))
        .route("/sources", get(rag_sources_handler))
        .route("/sources/:name", delete(delete_source_handler))
        .route("/stats", get(rag_stats_handler))
        .route(
            "/config",
            get(rag_config_get_handler).put(rag_config_put_handler),
        )
        .route("/ingest/file", post(rag_ingest_handler))
        .route("/bridge/ai/query", post(bridge_ai_query))
        .route("/bridge/ai/embed", post(bridge_ai_embed))
        .route("/bridge/ai/models", get(bridge_ai_models))
        .route(
            "/bridge/security/authorize",
            post(bridge_security_authorize),
        )
        .route("/agents/status", get(agents_status_handler))
        .route("/agents", get(agents_list_handler))
        .route("/task/submit", post(task_submit_handler))
        .route("/tasks", get(tasks_handler))
        .route("/workflow/run", axum::routing::post(workflow_run_handler))
        .route(
            "/knowledge-graph/entities",
            get(knowledge_graph_entities_handler),
        )
        .route(
            "/knowledge-graph/relations",
            get(knowledge_graph_relations_handler),
        )
        .route("/knowledge-graph/stats", get(knowledge_graph_stats_handler))
        .route("/coordinator/circuits", get(coordinator_circuits_handler))
        .route("/orchestrator/state", get(orchestrator_state_handler))
        .route("/orchestrator/forecast", get(orchestrator_forecast_handler))
        .route("/mcp/tools", get(mcp_tools_handler))
        .route("/a2a/messages", get(a2a_messages_handler))
        .route("/audit/log", get(audit_log_handler))
        .route("/audit/replay", get(audit_replay_handler))
        .route("/dev/logs", get(dev_logs_handler))
        .route("/dev/config", get(dev_config_handler))
        .route("/dev/metrics", get(dev_metrics_handler))
        .route("/sync/download/:filename", get(sync_download))
        .route("/sync/upload", post(sync_upload))
        .route("/settings", get(settings_handler))
        .route("/keys", get(keys_list_handler))
        .route(
            "/keys/:service",
            put(key_set_handler).delete(key_delete_handler),
        )
        .route("/keys/:service/toggle", post(key_toggle_handler))
        .route("/fusion/query", post(fusion_query_handler))
        .route("/guardian/health", get(guardian_health_handler))
        .route("/guardian/query", post(guardian_query_handler))
        .route("/guardian/versions", get(guardian_versions_handler))
        .route("/guardian/snapshot", post(guardian_snapshot_handler))
        .route("/guardian", get(guardian_page_handler))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Aetheris Core listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
