use axum::{
    body::{Body, Bytes},
    extract::{Host, Multipart, Path, Query, State},
    http::{header, HeaderMap, Method, Request, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;

mod a2a;
mod agents;
mod auth;
mod bridge;
mod config;
mod fusion;
mod guardian;
mod implementation;
mod kg;
mod mcp;
mod metrics;
mod proxy;
mod rag;
mod store;
mod store_api;
mod sync;
mod util;
mod wal;
mod watcher;

use a2a::A2AGateway;
use agents::Agent;
use bridge::{ModelBridge, SecurityBridge};
use fusion::{ExaSearchBridge, FusionRouter, KeyManager, OpenRouterBridge};
use guardian::Guardian;
use kg::KnowledgeGraph;
use mcp::MCPServer;
use proxy::{OpenAiProxy, OrchestratorProxy};
use rag::VectorStore;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
}

pub struct AppState {
    pub vault_path: std::path::PathBuf,
    pub web_root: std::path::PathBuf,
    pub security_watcher: Arc<watcher::SecurityWatcher>,
    pub ai_url: String,
    pub opa_url: String,
    pub opa_enforce: bool,
    pub cf_jwt: auth::cf_jwt::CfJwtConfig,
    pub port_registry: serde_json::Value,
    pub dev_logs: Mutex<Vec<LogEntry>>,
    pub wal: Arc<Mutex<wal::WriteAheadLog>>,
    pub model_bridge: Arc<dyn ModelBridge>,
    pub security_bridge: Arc<dyn SecurityBridge>,
    pub default_model: String,
    pub a2a_gateway: Mutex<A2AGateway>,
    pub mcp_server: MCPServer,
    pub agents: Mutex<Vec<Box<dyn Agent>>>,
    pub orchestrator_proxy: Option<OrchestratorProxy>,
    pub openai_proxy: OpenAiProxy,
    pub key_manager: Arc<KeyManager>,
    pub vector_store: Option<Arc<VectorStore>>,
    pub fusion_router: Option<FusionRouter>,
    pub guardian: Arc<Guardian>,
    pub rag_config: Arc<Mutex<rag::RagConfig>>,
    pub knowledge_graph: Option<Arc<KnowledgeGraph>>,
    pub store: Option<store::Store>,
    pub start_time: std::time::Instant,
}

// ─── Dashboard ───────────────────────────────────────────────────

async fn dashboard_handler() -> impl IntoResponse {
    let html = std::include_str!("../ui/index.html");
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap()
}

/// Serves the per-subdomain web panel from `WEB_ROOT` (e.g. `ai.nrupalakolkar.com`
/// -> `{WEB_ROOT}/ai/index.html`). Falls back to the compiled-in dashboard when the
/// host does not map to a panel (local health checks, apex, unknown hosts).
async fn web_index_handler(State(state): State<Arc<AppState>>, host: Host) -> impl IntoResponse {
    let hostname = host.0.trim().trim_end_matches('.').to_ascii_lowercase();
    let rel = web_panel_dir(&hostname);
    let index_path = state.web_root.join(rel).join("index.html");
    match tokio::fs::read(&index_path).await {
        Ok(bytes) => {
            let mut response = Response::new(Body::from(bytes));
            *response.status_mut() = StatusCode::OK;
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("text/html; charset=utf-8"),
            );
            response
        }
        Err(_) => dashboard_handler().await.into_response(),
    }
}

/// Maps a request hostname to a subdirectory of `WEB_ROOT`. `ai.*` -> `ai`,
/// `rag.*` -> `rag`, `agents.*` -> `agents`, `dev.*` -> `dev`, `guardian.*` ->
/// `guardian`, `settings.*` -> `settings`; apex/oracle and anything else -> ""
/// (i.e. `{WEB_ROOT}/index.html`).
fn web_panel_dir(hostname: &str) -> String {
    let host = hostname
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(hostname);
    for panel in ["ai", "rag", "agents", "dev", "guardian", "settings"] {
        if host == panel || host.starts_with(&format!("{}.", panel)) {
            return panel.to_string();
        }
    }
    String::new()
}

/// OpenAI-compatible `/v1/*` reverse proxy to the local Ollama backend. Paths are
/// forwarded to `{AI_ENDPOINT}/v1/...` and the upstream body is streamed back so
/// `stream: true` responses work. `/v1/models` is handled by the core route
/// (a specific route shadows this wildcard).
async fn v1_proxy_handler(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let path = uri
        .path()
        .strip_prefix("/v1")
        .filter(|p| !p.is_empty())
        .unwrap_or("/");
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    state
        .openai_proxy
        .forward(
            method,
            path,
            uri.query(),
            content_type.as_deref(),
            accept.as_deref(),
            body,
        )
        .await
}

// ─── Status ──────────────────────────────────────────────────────

async fn status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let core_port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    // Honest component state — none of this is invented.
    let vault_status = if state.vault_path.exists() {
        "present"
    } else {
        "missing"
    };
    let vector_db_status = if state.vector_store.is_some() {
        "connected"
    } else {
        "unavailable"
    };
    let kg_status = if state.knowledge_graph.is_some() {
        "connected"
    } else {
        "unavailable"
    };
    let banned_peers = state.security_watcher.banned_count();

    axum::Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": uptime,
        "port": core_port,
        "components": {
            "vault": {
                "status": vault_status,
                "encryption_at_rest": "none",
                "note": "native deployment; filesystem-level encryption (LUKS/ZFS) not configured"
            },
            "mesh": {
                "status": "disabled",
                "peers": 0,
                "note": "WireGuard mesh deferred; single node reached via Cloudflare Tunnel"
            },
            "ai": { "status": "configured" },
            "vector_db": { "status": vector_db_status },
            "knowledge_graph": { "status": kg_status }
        },
        "persistence": {
            "store": if state.store.is_some() { "active" } else { "disabled" },
            "path": state.store.as_ref().and_then(|s| s.path.clone()).map(|p| p.display().to_string())
        },
        "security": {
            "auto_ban": "active",
            "banned_peers": banned_peers,
            "ghost_shell": "disabled",
            "note": "ghost_shell honeypot is aspirational/not deployed; disabled"
        }
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

/// Accepts only a plain file basename (no path separators, ".", "..", empty)
/// and returns the vault-absolute path, or `None` if the name is unsafe.
///
/// This is the hardened guard for *write* paths. Unlike the read-side
/// `starts_with(vault)` check (which can lexically pass `dir/../../x`), a
/// strict basename check makes traversal to outside the vault impossible for
/// uploads.
fn vault_upload_path(vault: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
    {
        return None;
    }
    Some(vault.join(name))
}

async fn download_file(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let path = state.vault_path.join(&filename);
    if !path.starts_with(&state.vault_path) {
        push_dev_log(
            &state,
            "WARN",
            &format!("File download DENIED: {}", filename),
        );
        return (StatusCode::FORBIDDEN, "Access Denied").into_response();
    }
    match tokio::fs::File::open(&path).await {
        Ok(file) => {
            push_dev_log(&state, "INFO", &format!("File download: {}", filename));
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
        Err(_) => {
            push_dev_log(
                &state,
                "WARN",
                &format!("File download NOT FOUND: {}", filename),
            );
            (StatusCode::NOT_FOUND, "File not found").into_response()
        }
    }
}

async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded = 0;
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("unknown").to_string();
        let Some(file_path) = vault_upload_path(&state.vault_path, &name) else {
            push_dev_log(
                &state,
                "WARN",
                &format!("File upload DENIED (unsafe name): {}", name),
            );
            continue;
        };
        if let Ok(data) = field.bytes().await {
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
    push_dev_log(
        &state,
        "INFO",
        &format!("File upload: {} file(s) saved", uploaded),
    );
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
    if query.trim().is_empty() {
        return axum::Json(serde_json::json!({
            "query": query, "results": [], "total": 0, "note": "empty query"
        }))
        .into_response();
    }
    // Real semantic search over the vector store — not a hardcoded result.
    if let Some(ref store) = state.vector_store {
        match state.model_bridge.embed(&query).await {
            Ok(emb) => match store.search(&emb, 10) {
                Ok(results) => {
                    let items: Vec<serde_json::Value> = results
                        .iter()
                        .map(|r| {
                            serde_json::json!({
                                "filename": r.source,
                                "score": r.score,
                                "excerpt": r.text,
                                "chunk_index": r.chunk_index,
                                "chunk_id": r.chunk_id,
                            })
                        })
                        .collect();
                    return axum::Json(serde_json::json!({
                        "query": query, "results": items, "total": items.len()
                    }))
                    .into_response();
                }
                Err(e) => {
                    return axum::Json(serde_json::json!({
                        "query": query, "results": [], "total": 0, "error": e
                    }))
                    .into_response();
                }
            },
            Err(e) => {
                return axum::Json(serde_json::json!({
                    "query": query,
                    "results": [],
                    "total": 0,
                    "error": format!("embedding failed: {}", e),
                }))
                .into_response();
            }
        }
    }
    axum::Json(serde_json::json!({
        "query": query,
        "results": [],
        "total": 0,
        "note": "vector store not initialized",
    }))
    .into_response()
}

/// Returns (total_mb, used_mb) of system memory from /proc/meminfo (Linux).
/// Returns (0, 0) on non-Linux platforms where meminfo is unavailable.
fn system_memory_mb() -> (u64, u64) {
    let Ok(info) = std::fs::read_to_string("/proc/meminfo") else {
        return (0, 0);
    };
    let mut total_kb = 0u64;
    let mut avail_kb = 0u64;
    for line in info.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total_kb = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            avail_kb = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        }
    }
    if total_kb == 0 {
        return (0, 0);
    }
    let total_mb = total_kb / 1024;
    let used_mb = total_kb.saturating_sub(avail_kb) / 1024;
    (total_mb, used_mb)
}

async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ai_ok = state.model_bridge.list_models().await.is_ok();
    let agents = state.agents.lock().unwrap();
    let agents_len = agents.len();
    let active_tasks = agents
        .iter()
        .filter(|a| a.state().as_str() != "idle")
        .count();
    drop(agents);
    let kg_ok = state.knowledge_graph.is_some();
    let vs_ok = state.vector_store.is_some();
    let (total_mb, used_mb) = system_memory_mb();
    let memory_util_pct = used_mb
        .saturating_mul(100)
        .checked_div(total_mb)
        .unwrap_or(0);
    let bottleneck = if memory_util_pct > 80 {
        "memory"
    } else if active_tasks > 0 {
        "agents"
    } else {
        "none"
    };
    // Forecast confidence reflects whether we have a real system measurement:
    // real memory data -> high confidence; no data -> low.
    let confidence = if total_mb > 0 { 0.8_f64 } else { 0.2_f64 };
    let mut services = vec![
        serde_json::json!({"name": "aetheris_core", "status": if ai_ok { "running" } else { "degraded" }, "port": 8080}),
        serde_json::json!({"name": "vector_store", "status": if vs_ok { "running" } else { "unavailable" }, "port": 0}),
        serde_json::json!({"name": "knowledge_graph", "status": if kg_ok { "running" } else { "unavailable" }, "port": 0}),
    ];
    if state.orchestrator_proxy.is_some() {
        services.push(
            serde_json::json!({"name": "orchestrator_proxy", "status": "running", "port": 9090}),
        );
    }
    let tool_count = state.mcp_server.list_tools_json()["tools"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    axum::Json(serde_json::json!({
        "status": "ok",
        "services": services,
        "agents": agents_len,
        "tasks": active_tasks,
        "tools": tool_count,
        "prompts": 0,
        "ai_connected": ai_ok,
        "cross_system": state.orchestrator_proxy.is_some(),
        "total_memory_mb": total_mb,
        "memory_used_mb": used_mb,
        "spread_forecast": {
            "total_memory_mb": total_mb,
            "memory_utilization_pct": memory_util_pct,
            "confidence": confidence,
            "bottleneck": bottleneck
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
    let start = std::time::Instant::now();
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
                    } else {
                        push_dev_log(
                            &state,
                            "WARN",
                            &format!(
                                "Reranker unavailable ({}): falling back to vector search order",
                                cfg.reranker_model
                            ),
                        );
                    }
                }
                chunks.truncate(top_k);
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
        let code_hint = if query.contains("fn ")
            || query.contains("function ")
            || query.contains("class ")
            || query.contains("impl ")
            || query.contains("struct ")
            || query.contains("trait ")
            || query.contains("enum ")
            || query.contains("def ")
            || query.contains("pub ")
            || query.contains("unsafe ")
            || query.contains("mut ")
            || query.contains("async ")
            || query.contains(".await")
            || query.contains("-> ")
            || query.contains("=>")
            || query.contains("::")
            || query.contains("&self")
            || query.contains("&mut self")
            || query.contains("Result<")
            || query.contains("Option<")
            || query.contains("let ")
            || query.contains("match ")
            || query.contains("if let ")
            || query.contains("for ")
            || query.contains("while ")
            || query.contains("loop ")
            || query.contains("Vec<")
            || query.contains("HashMap<")
            || query.contains("String")
            || query.contains("&str")
        {
            " The context contains code files. Reference specific functions, types, and file paths when answering. Use code snippets in your answer when relevant."
        } else {
            ""
        };
        format!(
            "Context:\n{}\n\nQuestion: {}\n\nAnswer based on the context above.{}{}\n\nReturn ONLY valid JSON (no markdown, no code fences) where \"answer\" is a concise string (2-4 sentences), \"sources\" is a list of source filenames, and \"confidence\" is 0.0-1.0{}}}.",
            context, query, reasoning, code_hint,
            if use_reasoning { ", \"reasoning\" (string)" } else { "" }
        )
    } else {
        format!("Question: {}\n\nAnswer the question. Return ONLY valid JSON (no markdown, no code fences) {{\"answer\", \"sources\", \"confidence\"{}}}.", query, if use_reasoning { ", \"reasoning\"" } else { "" })
    };

    match state
        .model_bridge
        .query_with_timeout(&prompt, &cfg.query_model, cfg.timeout_secs, Some(256))
        .await
    {
        Ok(response) => {
            let cleaned = response
                .trim_start_matches("```json\n")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();
            let parsed: serde_json::Value = serde_json::from_str(cleaned).unwrap_or_else(
                |_| serde_json::json!({"answer": response, "sources": [], "confidence": 0.5}),
            );
            let sources_val = parsed
                .get("sources")
                .cloned()
                .unwrap_or(serde_json::json!([]));
            let src = if sources_val.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                context_chunks
                    .as_ref()
                    .map(|chunks| {
                        serde_json::json!(chunks
                            .iter()
                            .map(|c| serde_json::json!({"source": c.source, "score": c.score}))
                            .collect::<Vec<_>>())
                    })
                    .unwrap_or(serde_json::json!([]))
            } else {
                sources_val
            };
            let took_ms = start.elapsed().as_millis();
            let answer_preview = parsed
                .get("answer")
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                })
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    if response.trim_start().starts_with('{') && response.contains("\"answer\"") {
                        "No answer could be generated for this query. Try rephrasing, or check that relevant documents are indexed.".to_string()
                    } else {
                        response.clone()
                    }
                });
            let conf = parsed
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);
            push_dev_log(
                &state,
                "INFO",
                &format!(
                    "RAG query: \"{:.80}...\" → confidence {:.2}, {} chunks, {}ms",
                    query,
                    conf,
                    context_chunks.as_ref().map(|c| c.len()).unwrap_or(0),
                    took_ms
                ),
            );
            axum::Json(serde_json::json!({
                "query": query,
                "answer": answer_preview,
                "model": cfg.query_model,
                "sources": src,
                "confidence": conf,
                "top_k": top_k,
                "reasoning": parsed.get("reasoning").and_then(|v| v.as_str()).unwrap_or(""),
                "chunks_searched": context_chunks.as_ref().map(|c| c.len()).unwrap_or(0),
                "reranker_used": use_reranker,
                "reranker_model": cfg.reranker_model,
                "took_ms": took_ms,
            }))
            .into_response()
        }
        Err(e) => {
            push_dev_log(
                &state,
                "ERROR",
                &format!("RAG query failed: \"{:.80}...\" → {}", query, e),
            );
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": e, "query": query, "answer": "", "sources": [], "confidence": 0.0
                })),
            )
                .into_response()
        }
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
                    "source": s.source, "chunks": s.chunk_count,
                    "first_seen": s.first_seen, "last_seen": s.last_seen
                })
            })
            .collect()
    } else {
        vec![]
    };

    let indexed: std::collections::HashSet<String> = sources
        .iter()
        .filter_map(|s| s["source"].as_str().map(String::from))
        .collect();

    if let Ok(mut entries) = tokio::fs::read_dir(&state.vault_path).await {
        loop {
            match entries.next_entry().await {
                Ok(Some(entry)) => {
                    let path = entry.path();
                    if path.is_file() {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if is_vault_artifact(&name) || indexed.contains(&name) {
                            continue;
                        }
                        if let Ok(meta) = entry.metadata().await {
                            sources.push(serde_json::json!({
                                "source": name,
                                "size": meta.len(),
                                "type": path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default(),
                                "chunks": 0,
                                "last_seen": ""
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

fn is_vault_artifact(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "rag_config.json"
        || lower.ends_with(".db")
        || lower.ends_with(".db-wal")
        || lower.ends_with(".db-shm")
        || lower.ends_with(".wal")
        || lower.ends_with(".shm")
}

async fn delete_source_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let path = state.vault_path.join(&name);
    if !path.starts_with(&state.vault_path) {
        return (StatusCode::FORBIDDEN, "Access Denied").into_response();
    }
    let removed = tokio::fs::remove_file(&path).await.is_ok();
    let mut purged = 0usize;
    if let Some(ref store) = state.vector_store {
        purged = store.delete_source(&name).unwrap_or(0);
    }
    if removed || purged > 0 {
        push_dev_log(
            &state,
            "INFO",
            &format!("Source deleted: {} ({} chunks purged)", name, purged),
        );
        state
            .wal
            .lock()
            .unwrap()
            .append(wal::WalEntry::FileDelete {
                filename: name.clone(),
            })
            .ok();
        axum::Json(serde_json::json!({"status": "deleted", "name": name, "chunks_purged": purged}))
            .into_response()
    } else {
        push_dev_log(&state, "WARN", &format!("File delete NOT FOUND: {}", name));
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"status": "not_found", "name": name})),
        )
            .into_response()
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
                        let name = entry.file_name().to_string_lossy().to_string();
                        if is_vault_artifact(&name) {
                            continue;
                        }
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
    let cfg = state.rag_config.lock().unwrap().clone();
    axum::Json(serde_json::json!({
        "documents": file_count,
        "total_chunks": store_stats.as_ref().map(|s| s.total_chunks).unwrap_or(file_count as i64 * 3),
        "total_size_bytes": total_size,
        "indexed_vectors": store_stats.as_ref().map(|s| s.total_chunks).unwrap_or(file_count as i64),
        "embedding_dimension": store_stats.as_ref().map(|s| s.embedding_dimension).unwrap_or(0),
        "total_sources": store_stats.as_ref().map(|s| s.total_sources).unwrap_or(0),
        "db_size_mb": store_stats.as_ref().map(|s| s.db_size_mb).unwrap_or(0.0),
        "collections": 1,
        "avg_chunk_size": cfg.chunk_size,
    })).into_response()
}

async fn rag_config_get_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cfg = state.rag_config.lock().unwrap().clone();
    axum::Json(serde_json::json!(cfg))
}

async fn rag_config_put_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let current = state.rag_config.lock().unwrap().clone();
    let Some(map) = payload.as_object() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Expected a JSON object"})),
        )
            .into_response();
    };
    let mut merged = serde_json::to_value(&current).unwrap_or_default();
    let Some(merged_map) = merged.as_object_mut() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Failed to serialize current config"})),
        )
            .into_response();
    };
    for (k, v) in map {
        merged_map.insert(k.clone(), v.clone());
    }
    let updated = match serde_json::from_value::<rag::RagConfig>(merged) {
        Ok(cfg) => cfg,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid config: {}", e)})),
            )
                .into_response();
        }
    };
    {
        let mut cfg = state.rag_config.lock().unwrap();
        *cfg = updated.clone();
        cfg.save(&state.vault_path);
    }
    push_dev_log(
        &state,
        "INFO",
        &format!("RAG config saved: model={}", updated.query_model),
    );
    let cfg = state.rag_config.lock().unwrap().clone();
    axum::Json(serde_json::json!({"status": "saved", "config": cfg})).into_response()
}

fn supported_ingest_ext(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".txt")
        || lower.ends_with(".md")
        || lower.ends_with(".json")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".csv")
        || lower.ends_with(".xml")
        || lower.ends_with(".html")
        || lower.ends_with(".htm")
        || lower.ends_with(".rs")
        || lower.ends_with(".py")
        || lower.ends_with(".js")
        || lower.ends_with(".ts")
        || lower.ends_with(".toml")
        || lower.ends_with(".pdf")
        // code files
        || lower.ends_with(".c")
        || lower.ends_with(".h")
        || lower.ends_with(".cpp")
        || lower.ends_with(".hpp")
        || lower.ends_with(".cc")
        || lower.ends_with(".cxx")
        || lower.ends_with(".go")
        || lower.ends_with(".ada")
        || lower.ends_with(".adb")
        || lower.ends_with(".ads")
        || lower.ends_with(".java")
        || lower.ends_with(".sql")
        || lower.ends_with(".m")
        || lower.ends_with(".sh")
        || lower.ends_with(".bash")
        || lower.ends_with(".zsh")
        || lower.ends_with(".rb")
        || lower.ends_with(".php")
        || lower.ends_with(".swift")
        || lower.ends_with(".kt")
        || lower.ends_with(".kts")
        || lower.ends_with(".lua")
        || lower.ends_with(".r")
        || lower.ends_with(".dart")
}

async fn rag_ingest_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded = 0u64;
    let mut total_chunks = 0usize;
    let mut errors: Vec<String> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("unknown").to_string();

        if !supported_ingest_ext(&name) {
            errors.push(format!(
                "\"{}\": unsupported file type — only text formats (.txt, .md, .csv, .json, .html, .rs, .py, etc.) are supported",
                name
            ));
            continue;
        }

        let Some(file_path) = vault_upload_path(&state.vault_path, &name) else {
            errors.push(format!("\"{}\": unsafe file name rejected", name));
            continue;
        };

        if let Ok(data) = field.bytes().await {
            if tokio::fs::write(&file_path, &data[..]).await.is_ok() {
                uploaded += 1;

                if let Some(ref store) = state.vector_store {
                    let is_pdf = name.to_lowercase().ends_with(".pdf");
                    let content: String = if is_pdf {
                        match pdf_extract::extract_text(&file_path) {
                            Ok(t) => t,
                            Err(e) => {
                                errors.push(format!(
                                    "\"{}\": failed to extract text from PDF — {}",
                                    name, e
                                ));
                                continue;
                            }
                        }
                    } else {
                        match String::from_utf8(data.to_vec()) {
                            Ok(c) => c,
                            Err(_) => {
                                errors.push(format!(
                                    "\"{}\": file contains non-text binary data",
                                    name
                                ));
                                continue;
                            }
                        }
                    };
                    let cfg = state.rag_config.lock().unwrap().clone();
                    let chunker = rag::TextChunker::new(cfg.chunk_size, cfg.chunk_overlap);
                    let chunks = chunker.chunk(&content, &name);
                    if chunks.is_empty() {
                        errors.push(format!(
                            "\"{}\": file produced 0 chunks after text extraction",
                            name
                        ));
                        continue;
                    }
                    let mut embeddings = Vec::new();
                    let mut embed_ok = true;
                    for chunk in &chunks {
                        match state.model_bridge.embed(&chunk.text).await {
                            Ok(emb) => embeddings.push(emb),
                            Err(e) => {
                                errors.push(format!("\"{}\": embedding failed — {}", name, e));
                                embed_ok = false;
                                break;
                            }
                        }
                    }
                    if embed_ok && !embeddings.is_empty() {
                        match store.add_chunks(&chunks, &embeddings) {
                            Ok(ids) => {
                                total_chunks += ids.len();
                                if let Some(ref kg) = state.knowledge_graph {
                                    for chunk in &chunks {
                                        let _ = kg.ingest(&chunk.text, &name);
                                    }
                                }
                                state
                                    .wal
                                    .lock()
                                    .unwrap()
                                    .append(wal::WalEntry::Custom {
                                        action: "rag_ingest".to_string(),
                                        details: format!("file={}, chunks={}", name, ids.len()),
                                    })
                                    .ok();
                            }
                            Err(e) => {
                                errors
                                    .push(format!("\"{}\": failed to store chunks — {}", name, e));
                            }
                        }
                    }
                }
            }
        }
    }

    let status = if uploaded == 0 {
        "error"
    } else if total_chunks == 0 {
        "warning"
    } else {
        "success"
    };

    let mut resp = serde_json::json!({
        "status": status,
        "files_uploaded": uploaded,
        "chunks_indexed": total_chunks,
        "message": if !errors.is_empty() {
            errors.join("; ")
        } else {
            format!("Uploaded {} file(s) and indexed {} chunk(s)", uploaded, total_chunks)
        }
    });

    if !errors.is_empty() {
        resp["warnings"] = serde_json::json!(errors);
    }

    axum::Json(resp).into_response()
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
        .unwrap_or(&state.default_model);
    let peer_id = payload
        .get("peer_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("query");

    let authz_input = bridge::AuthzInput {
        identity: peer_id.to_string(),
        role: "unknown".to_string(),
        method: "POST".to_string(),
        path: "/bridge/ai/query".to_string(),
        action: action.to_string(),
    };
    let allowed = state.security_bridge.authorize(&authz_input).await;
    if !allowed {
        log::warn!(
            "OPA would DENY {} {} (identity={})",
            authz_input.method,
            authz_input.path,
            authz_input.identity
        );
        metrics::SECURITY_VIOLATIONS.inc();
        // Only enforce when OPA enforcement is enabled (Phase 3 default off) so
        // standing up OPA does not 403 this path while the identity/role contract
        // is still being wired.
        if state.opa_enforce {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "Policy denied"})),
            )
                .into_response();
        }
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
    let authz_input = bridge::AuthzInput {
        identity: peer_id.to_string(),
        role: "unknown".to_string(),
        method: "POST".to_string(),
        path: "/bridge/security/authorize".to_string(),
        action: action.to_string(),
    };
    let allowed = state.security_bridge.authorize(&authz_input).await;
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

    // Persist the workflow as a task + conversation row.
    let task_id = state.store.as_ref().and_then(|s| {
        s.create_task(Some(task), "workflow", "pipeline", "running")
            .ok()
    });
    if let Some(store) = state.store.as_ref() {
        let _ = store.create_conversation(&conv_id, task, "unknown");
        let _ = store.append_message(&conv_id, "user", task);
    }

    let agent_roles: Vec<String> = {
        let a = state.agents.lock().unwrap();
        a.iter().map(|ag| ag.role().as_str().to_string()).collect()
    };

    let pipeline = ["planner", "researcher", "coder"];
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
                            task,
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

    // Close out persisted task + conversation with the true outcome.
    if let Some(store) = state.store.as_ref() {
        if let Some(id) = task_id {
            if all_ok {
                let _ = store.update_task(id, "completed", Some(&final_output), None);
            } else {
                let _ = store.update_task(id, "failed", Some(&final_output), None);
            }
        }
        let _ = store.append_message(&conv_id, "assistant", &final_output);
    }

    push_dev_log(
        &state,
        "INFO",
        &format!(
            "Agent workflow: \"{:.80}...\" → {} steps, {}ms, success={}",
            task,
            steps_executed.len(),
            total_duration as u64,
            all_ok
        ),
    );

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
        .and_then(|s| s.parse::<agents::AgentRole>().ok())
        .unwrap_or(agents::AgentRole::Planner);
    let ctx = payload.context.unwrap_or(serde_json::json!({}));
    let role_str = agent_role.as_str().to_string();

    // Persist the submission as a task row before execution (request_id = task
    // text; the row is tracked through the whole run).
    let task_id = state
        .store
        .as_ref()
        .and_then(|s| s.create_task(Some(&task), "", &role_str, "running").ok());

    let mut agent = agents::create_agent(
        agent_role,
        state.default_model.clone(),
        String::new(),
        Some(state.model_bridge.clone()),
        Some(state.security_bridge.clone()),
    );

    let result = agent.execute(&task, &ctx).await;

    let mut agents = state.agents.lock().unwrap();
    agents.push(agent);

    if let (Some(store), Some(id)) = (state.store.as_ref(), task_id) {
        if result.success {
            let _ = store.update_task(id, "completed", Some(&result.output), None);
        } else {
            let _ = store.update_task(id, "failed", None, Some("agent execution failed"));
        }
    }

    axum::Json(serde_json::json!({
        "agent_id": result.agent_id, "output": result.output,
        "success": result.success, "duration_ms": result.duration_ms,
        "task_id": task_id,
    }))
}

async fn tasks_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Real, current agent-pool task rows — not a fabricated permanent empty list.
    let agents = state.agents.lock().unwrap();
    let tasks: Vec<serde_json::Value> = agents
        .iter()
        .map(|a| {
            serde_json::json!({
                "agent_id": a.id().to_string(),
                "role": a.role().as_str(),
                "status": a.state().as_str(),
                "tasks_completed": a.tasks_completed(),
                "policy_checks": a.policy_checks(),
                "policy_allowed": a.policy_allowed(),
            })
        })
        .collect();
    drop(agents);
    axum::Json(serde_json::json!(tasks)).into_response()
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
    let (total_mb, used_mb) = system_memory_mb();
    axum::Json(serde_json::json!({
        "forecast": {
            "total_memory_mb": total_mb,
            "memory_utilization_pct": used_mb.saturating_mul(100).checked_div(total_mb).unwrap_or(utilization),
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
    match state.knowledge_graph {
        Some(ref kg) => {
            match kg.get_entities(None, 500) {
                Ok(entities) => axum::Json(serde_json::json!({"entities": entities, "total": entities.len()})),
                Err(e) => axum::Json(serde_json::json!({"entities": [], "total": 0, "error": e})),
            }
        }
        None => {
            axum::Json(serde_json::json!({"entities": [], "total": 0, "note": "knowledge_graph not initialized"}))
        }
    }.into_response()
}

async fn knowledge_graph_relations_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.knowledge_graph {
        Some(ref kg) => {
            match kg.get_relations(500) {
                Ok(relations) => axum::Json(serde_json::json!({"relations": relations, "total": relations.len()})),
                Err(e) => axum::Json(serde_json::json!({"relations": [], "total": 0, "error": e})),
            }
        }
        None => {
            axum::Json(serde_json::json!({"relations": [], "total": 0, "note": "knowledge_graph not initialized"}))
        }
    }.into_response()
}

async fn knowledge_graph_stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.knowledge_graph {
        Some(ref kg) => {
            match kg.get_stats() {
                Ok(stats) => axum::Json(serde_json::json!(stats)),
                Err(e) => axum::Json(serde_json::json!({"entities": 0, "relations": 0, "clusters": 0, "central_nodes": [], "error": e})),
            }
        }
        None => {
            axum::Json(serde_json::json!({"entities": 0, "relations": 0, "clusters": 0, "central_nodes": [], "note": "knowledge_graph not initialized"}))
        }
    }.into_response()
}

async fn coordinator_circuits_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Real circuit state derived from actual component health.
    let (agent_total, failed_agents) = {
        let a = state.agents.lock().unwrap();
        (
            a.len() as u64,
            a.iter().filter(|x| x.state().as_str() == "failed").count() as u64,
        )
    };
    let circuits = vec![
        serde_json::json!({
            "name": "aetheris_core", "state": "closed", "failures": 0
        }),
        serde_json::json!({
            "name": "vector_store",
            "state": if state.vector_store.is_some() { "closed" } else { "open" },
            "failures": if state.vector_store.is_some() { 0 } else { 1 },
        }),
        serde_json::json!({
            "name": "knowledge_graph",
            "state": if state.knowledge_graph.is_some() { "closed" } else { "open" },
            "failures": if state.knowledge_graph.is_some() { 0 } else { 1 },
        }),
        serde_json::json!({
            "name": "agents",
            "state": if agent_total > 0 && failed_agents == agent_total { "open" } else { "closed" },
            "failures": failed_agents,
        }),
    ];
    axum::Json(serde_json::json!({"circuits": circuits})).into_response()
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
    let logs = state.dev_logs.lock().unwrap();
    let entries: Vec<serde_json::Value> = logs
        .iter()
        .map(|e| serde_json::json!({"timestamp": e.timestamp, "level": e.level, "message": e.message}))
        .collect();
    axum::Json(serde_json::json!({"logs": entries}))
}

fn push_dev_log(state: &AppState, level: &str, message: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    {
        let mut logs = state.dev_logs.lock().unwrap();
        logs.push(LogEntry {
            timestamp: ts,
            level: level.to_string(),
            message: message.to_string(),
        });
    }
    if let Ok(mut wal) = state.wal.lock() {
        let _ = wal.append(wal::WalEntry::DevLog {
            level: level.to_string(),
            message: message.to_string(),
        });
    }
}

async fn dev_log_append_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let level = payload
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("INFO");
    let message = payload
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if message.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "message is required"})),
        )
            .into_response();
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let entry = LogEntry {
        timestamp: ts,
        level: level.to_string(),
        message: message.to_string(),
    };
    {
        let mut logs = state.dev_logs.lock().unwrap();
        logs.push(entry);
    }
    {
        let mut wal = state.wal.lock().unwrap();
        let _ = wal.append(wal::WalEntry::DevLog {
            level: level.to_string(),
            message: message.to_string(),
        });
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "ok", "timestamp": ts})),
    )
        .into_response()
}

async fn dev_config_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut files: HashMap<String, String> = HashMap::new();
    let config_dir = state.vault_path.join("config");
    if config_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&config_dir) {
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
    }
    axum::Json(files)
}

/// Probes whether a loopback TCP port answers (used to report real live
/// services instead of fabricated state on the native (no-Docker) deployment).
async fn probe_service_port(port: u16) -> bool {
    tokio::time::timeout(
        std::time::Duration::from_millis(300),
        tokio::net::TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

async fn dev_metrics_handler(state: State<Arc<AppState>>) -> impl IntoResponse {
    let uptime_hours = state.start_time.elapsed().as_secs_f64() / 3600.0;

    // Native systemd deployment has no Docker containers — report that honestly.
    let containers = serde_json::json!({
        "total": 0,
        "running": 0,
        "deployment": "native (no Docker)",
    });

    // Probe the actual loopback service suite to report real liveness.
    let suite: [(String, u16, String); 10] = [
        ("aetheris_core".into(), 8080, "Rust Axum API server".into()),
        ("ollama".into(), 11434, "Local LLM inference".into()),
        ("opa".into(), 8181, "Open Policy Agent".into()),
        ("aetheris_mgmt".into(), 9090, "Management UI".into()),
        ("oc_bridge".into(), 8888, "opencode MCP bridge".into()),
        ("opencode".into(), 8192, "opencode server".into()),
        ("guardian_agent".into(), 8081, "Guardian agent webui".into()),
        ("bee".into(), 8800, "Bee service".into()),
        (
            "research_analyst".into(),
            8700,
            "Research Analyst API".into(),
        ),
        ("code_server".into(), 8088, "VS Code server".into()),
    ];

    let mut handles = Vec::new();
    for (name, port, description) in suite {
        handles.push(tokio::spawn(async move {
            (name, port, description, probe_service_port(port).await)
        }));
    }
    let mut services = Vec::new();
    for h in handles {
        if let Ok((name, port, description, alive)) = h.await {
            services.push(serde_json::json!({
                "name": name,
                "port": port,
                "status": if alive { "running" } else { "stopped" },
                "description": description,
                "running": alive,
            }));
        }
    }
    services.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));

    axum::Json(serde_json::json!({
        "services": services,
        "containers": containers,
        "uptime_hours": uptime_hours,
        "deployment": "native",
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
        let Some(path) = vault_upload_path(&state.vault_path, &name) else {
            push_dev_log(
                &state,
                "WARN",
                &format!("Sync upload DENIED (unsafe name): {}", name),
            );
            continue;
        };
        if let Ok(data) = field.bytes().await {
            tokio::fs::write(&path, &data).await.unwrap_or_default();
        }
    }
    StatusCode::OK
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
        match state.model_bridge.query(query, &state.default_model).await {
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

/// Map Cloudflare Access headers to an OPA role string (pure, unit-tested).
///
/// * `email`: `Cf-Access-Authenticated-User-Email` (human users).
/// * `service_token_client_id`: `Cf-Access-Client-Id` (service-token automation).
fn access_role(email: &str, service_token_client_id: Option<&str>) -> &'static str {
    implementation::identity_to_role(
        if email.is_empty() { None } else { Some(email) },
        service_token_client_id,
    )
    .as_str()
}

/// True for state-changing verbs. Catches /ingest, /bridge/*, /keys writes,
/// /task, /workflow, /sync/upload etc. (the old blanket prefix list).
fn is_mutating(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH" | "DELETE")
}

/// Restrict OPA enforcement to sensitive requests (D2 scope): any mutating verb,
/// or a GET that reads secrets/logs/files. GET/static/panels stay under CF Access.
/// Method-aware - a bare GET to a write-capable path prefix is NOT sensitive.
fn is_sensitive(method: &str, path: &str) -> bool {
    is_mutating(method)
        || (method == "GET"
            && ["/keys", "/audit", "/sync/download", "/dev/logs"]
                .iter()
                .any(|p| path.starts_with(p)))
}

/// OPA shadow middleware (Phase 3). Evaluates every request against OPA and
/// logs + bumps the violation counter on would-deny, but only blocks when
/// `opa_enforce` is enabled on a sensitive route. `opa_enforce` is false this
/// phase, so nothing short-circuits.
async fn opa_gate(State(state): State<Arc<AppState>>, req: Request<Body>, next: Next) -> Response {
    let headers = req.headers();
    let email = headers
        .get("Cf-Access-Authenticated-User-Email")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let client_id = headers
        .get("Cf-Access-Client-Id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();

    // CF Access JWT verification on sensitive routes (P5).
    // - CF_JWT_VERIFY=1 (enforce): the VERIFIED JWT email is the authoritative identity;
    //   an unverifiable/missing/forged assertion degrades to `unknown` (denied on
    //   sensitive routes by OPA), closing the plaintext-header spoof gap.
    // - CF_JWT_VERIFY=0 (shadow): observe + log only; identity still from the header.
    // eprintln! so observations surface in journalctl -u aetheris-core (the app
    // installs logging via tracing, not the `log` facade).
    let mut identity_email = email.clone();
    if is_sensitive(&method, &path) {
        match auth::cf_jwt::verify_assertion(headers, &state.cf_jwt) {
            Ok(identity) => {
                if state.cf_jwt.enabled {
                    identity_email = identity.email;
                } else if !identity.email.is_empty() && !email.is_empty() && identity.email != email
                {
                    eprintln!(
                        "CFJWT shadow: header-email mismatch {} != {} on {} {}",
                        email, identity.email, method, path
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "CFJWT {}: verify failed on {} {} ({}): {}",
                    if state.cf_jwt.enabled {
                        "deny"
                    } else {
                        "shadow"
                    },
                    method,
                    path,
                    if email.is_empty() { "<none>" } else { &email },
                    e
                );
                if state.cf_jwt.enabled {
                    identity_email = String::new();
                }
            }
        }
    }

    let input = bridge::AuthzInput {
        identity: identity_email.clone(),
        role: access_role(&identity_email, client_id.as_deref()).to_string(),
        method: method.clone(),
        path: path.clone(),
        action: "http".to_string(),
    };

    let allowed = state.security_bridge.authorize(&input).await;
    if !allowed {
        log::warn!(
            "OPA would DENY {} {} ({})",
            input.method,
            input.path,
            if email.is_empty() { "<none>" } else { &email }
        );
        metrics::SECURITY_VIOLATIONS.inc();
        // Enforcement is intentionally off this phase: only 403 mutating/sensitive
        // routes when opa_enforce=true.
        if state.opa_enforce && is_sensitive(&method, &path) {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "Policy denied"})),
            )
                .into_response();
        }
    }
    next.run(req).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Aetheris Core Active. Zero-Trust Mesh Engaged.");

    let cfg = config::Config::from_env();
    let vault_path = cfg.vault_path.clone();
    let ai_url = cfg.ai_endpoint.clone();
    let opa_url = cfg.opa_endpoint.clone();
    let orch_url = std::env::var("ORCHESTRATOR_ENDPOINT").ok();
    let ai_model = std::env::var("AI_MODEL").unwrap_or_else(|_| cfg.fallback_model.clone());
    let embed_fallback_models = cfg.embed_fallback_model.clone();
    let rag_cfg_init = rag::RagConfig::load(&vault_path);
    let web_root = cfg.web_root.clone();
    let openai_proxy = OpenAiProxy::new(format!("{}/v1", cfg.ai_endpoint));

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
    ollama.default_model = ai_model.clone();
    ollama.embed_fallback_models = if rag_cfg_init.embed_models.is_empty() {
        vec![embed_fallback_models]
    } else {
        rag_cfg_init.embed_models.clone()
    };
    let model_bridge: Arc<dyn ModelBridge> = Arc::new(ollama);
    let security_bridge: Arc<dyn SecurityBridge> = Arc::new(implementation::OpaBridge::new(
        opa_url.clone(),
        cfg.opa_fail_open,
        cfg.opa_enforce,
    ));

    let orchestrator_proxy = orch_url.map(OrchestratorProxy::new);
    let a2a_gateway = A2AGateway::new(300);
    let mcp_server = MCPServer::new();

    let mut agent_vec: Vec<Box<dyn Agent>> = Vec::new();
    for (role, _model) in &[
        ("planner", ai_url.clone()),
        ("researcher", ai_url.clone()),
        ("coder", ai_url.clone()),
        ("reviewer", ai_url.clone()),
    ] {
        let r = role
            .parse::<agents::AgentRole>()
            .unwrap_or(agents::AgentRole::Researcher);
        agent_vec.push(agents::create_agent(
            r,
            ai_model.clone(),
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
        ai_model.clone(),
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
    let knowledge_graph = KnowledgeGraph::new(&vault_path.join("knowledge_graph.db")).ok();
    if knowledge_graph.is_none() {
        eprintln!("Warning: KnowledgeGraph failed to initialize (non-fatal)");
    }

    let store = store::Store::open(&cfg.store_path).ok();
    if store.is_some() {
        println!("Persistence store initialized at {:?}", cfg.store_path);
    } else {
        eprintln!(
            "Warning: persistence store failed to initialize at {:?} (non-fatal)",
            cfg.store_path
        );
    }
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

    let rag_config = Arc::new(Mutex::new(rag_cfg_init));
    println!(
        "RAG config loaded from {:?}",
        vault_path.join("rag_config.json")
    );

    let state = Arc::new(AppState {
        vault_path: vault_path.clone(),
        web_root: web_root.clone(),
        security_watcher: Arc::new(watcher::SecurityWatcher::new()),
        ai_url,
        opa_url,
        opa_enforce: cfg.opa_enforce,
        cf_jwt: auth::cf_jwt::CfJwtConfig {
            team_domain: cfg.cf_access_team_domain.clone(),
            aud: cfg.cf_access_aud.iter().cloned().collect(),
            jwks_path: cfg.cf_access_jwks_path.clone(),
            enabled: cfg.cf_jwt_verify,
        },
        default_model: ai_model,
        port_registry,
        wal: wal_arc.clone(),
        dev_logs: {
            let mut logs: Vec<LogEntry> = Vec::new();
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default();
            {
                let wal = wal_arc.lock().unwrap();
                let _ = wal.replay(|record| {
                    if let wal::WalEntry::DevLog { level, message } = &record.entry {
                        logs.push(LogEntry {
                            timestamp: record.timestamp,
                            level: level.clone(),
                            message: message.clone(),
                        });
                    }
                    Ok(())
                });
            }
            if logs.is_empty() {
                logs.push(LogEntry {
                    timestamp: ts,
                    level: "INFO".into(),
                    message: "Aetheris Core v0.1.0 starting up".into(),
                });
                logs.push(LogEntry {
                    timestamp: ts,
                    level: "INFO".into(),
                    message: "Zero-Trust Mesh Engaged".into(),
                });
                logs.push(LogEntry {
                    timestamp: ts,
                    level: "INFO".into(),
                    message: format!("{} agents initialized", agent_vec.len()),
                });
                logs.push(LogEntry {
                    timestamp: ts,
                    level: "INFO".into(),
                    message: "Vector store online".into(),
                });
                logs.push(LogEntry {
                    timestamp: ts,
                    level: "INFO".into(),
                    message: "Listening on 0.0.0.0:8080".into(),
                });
            }
            Mutex::new(logs)
        },
        model_bridge,
        security_bridge,
        a2a_gateway: Mutex::new(a2a_gateway),
        mcp_server,
        agents: Mutex::new(agent_vec),
        orchestrator_proxy,
        openai_proxy,
        key_manager,
        vector_store: vector_store.map(Arc::new),
        fusion_router: Some(fusion_router),
        guardian,
        rag_config,
        knowledge_graph: knowledge_graph.map(Arc::new),
        store,
        start_time: std::time::Instant::now(),
    });

    // All JSON/API routes, registered on both `/api/*` (dev-panel prefix) and the
    // bare path. `api_router` is nested under `/api` (which strips the prefix) and
    // merged at the root; both keep the same `Arc<AppState>` state, supplied by the
    // outer router's `.with_state(state)`.
    let api_router = Router::new()
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
        .route(
            "/dev/logs",
            get(dev_logs_handler).post(dev_log_append_handler),
        )
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
        .merge(store_api::store_router());

    let app = Router::new()
        .route("/", get(web_index_handler))
        .nest_service("/web", ServeDir::new(web_root))
        .nest("/api", api_router.clone())
        .merge(api_router)
        .route("/v1/*path", get(v1_proxy_handler).post(v1_proxy_handler))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            opa_gate,
        ))
        .with_state(state);

    let port = cfg.port.to_string();
    // Loopback-only: external access is exclusively via the cloudflared tunnel
    // (which connects from lo). Avoids depending on iptables + dashboard ingress
    // for header-trust. Consequence: all traffic is loopback-sourced, so internal
    // caller trust must be a service-token header, not a source-IP bypass.
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Aetheris Core listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("Shutdown signal received, starting graceful shutdown...");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{AetherisBridge, AuthzInput};
    use std::sync::Arc;
    use tower::ServiceExt;

    struct AllowBridge;
    struct DenyBridge;

    #[async_trait::async_trait]
    impl AetherisBridge for AllowBridge {
        fn name(&self) -> &str {
            "allow"
        }
        async fn health_check(&self) -> bool {
            true
        }
    }
    #[async_trait::async_trait]
    impl SecurityBridge for AllowBridge {
        async fn authorize(&self, _input: &AuthzInput) -> bool {
            true
        }
        async fn authorize_agent(&self, _input: &AuthzInput) -> bool {
            true
        }
        fn enforcing(&self) -> bool {
            false
        }
    }
    #[async_trait::async_trait]
    impl AetherisBridge for DenyBridge {
        fn name(&self) -> &str {
            "deny"
        }
        async fn health_check(&self) -> bool {
            true
        }
    }
    #[async_trait::async_trait]
    impl SecurityBridge for DenyBridge {
        async fn authorize(&self, _input: &AuthzInput) -> bool {
            false
        }
        async fn authorize_agent(&self, _input: &AuthzInput) -> bool {
            false
        }
        fn enforcing(&self) -> bool {
            false
        }
    }

    fn test_state_cf(
        bridge: Arc<dyn SecurityBridge>,
        opa_enforce: bool,
        cf_enabled: bool,
    ) -> Arc<AppState> {
        let dir = std::env::temp_dir().join(format!("aetheris_opa3_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let vault_dir = dir.join("vault");
        std::fs::create_dir_all(&vault_dir).ok();
        let wal_dir = dir.join("wal");
        std::fs::create_dir_all(&wal_dir).ok();

        let ai_url = "http://127.0.0.1:1".to_string();
        let opa_url = "http://127.0.0.1:1".to_string();
        let model_bridge: Arc<dyn ModelBridge> =
            Arc::new(implementation::OllamaBridge::new(ai_url.clone()));
        let wal_arc: Arc<Mutex<wal::WriteAheadLog>> = Arc::new(Mutex::new(
            wal::WriteAheadLog::new(&wal_dir.to_string_lossy()).expect("temp WAL"),
        ));
        let guardian = Arc::new(Guardian::new(
            model_bridge.clone(),
            None,
            None,
            wal_arc.clone(),
            vault_dir.clone(),
        ));

        Arc::new(AppState {
            vault_path: vault_dir.clone(),
            web_root: std::env::temp_dir(),
            security_watcher: Arc::new(watcher::SecurityWatcher::new()),
            ai_url: ai_url.clone(),
            opa_url,
            opa_enforce,
            cf_jwt: auth::cf_jwt::CfJwtConfig {
                team_domain: "https://nrupal.cloudflareaccess.com".to_string(),
                aud: std::collections::HashSet::new(),
                jwks_path: std::env::temp_dir().join("missing_jwks.json"),
                enabled: cf_enabled,
            },
            port_registry: serde_json::json!({}),
            dev_logs: Mutex::new(Vec::new()),
            wal: wal_arc,
            model_bridge,
            security_bridge: bridge,
            default_model: "test".to_string(),
            a2a_gateway: Mutex::new(A2AGateway::new(10)),
            mcp_server: MCPServer::new(),
            agents: Mutex::new(Vec::new()),
            orchestrator_proxy: None,
            openai_proxy: OpenAiProxy::new(format!("{}/v1", ai_url)),
            key_manager: Arc::new(KeyManager::new(&vault_dir)),
            vector_store: None,
            fusion_router: None,
            guardian,
            rag_config: Arc::new(Mutex::new(rag::RagConfig::default())),
            knowledge_graph: None,
            store: None,
            start_time: std::time::Instant::now(),
        })
    }

    async fn test_app(bridge: Arc<dyn SecurityBridge>, opa_enforce: bool) -> Router {
        test_app_cf(bridge, opa_enforce, false).await
    }

    async fn test_app_cf(
        bridge: Arc<dyn SecurityBridge>,
        opa_enforce: bool,
        cf_enabled: bool,
    ) -> Router {
        let state = test_state_cf(bridge, opa_enforce, cf_enabled);
        let handler = || async { "hello" };
        Router::new()
            .route("/sensitive", axum::routing::post(handler))
            .route("/panel", axum::routing::get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                opa_gate,
            ))
            .with_state(state)
    }

    #[tokio::test]
    async fn shadow_passes_through_on_would_deny() {
        let app = test_app(Arc::new(DenyBridge), false).await;
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/panel")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap(),
            "hello"
        );
    }

    #[tokio::test]
    async fn would_deny_logs_bumps_metric_but_returns_normal_response() {
        let before = metrics::SECURITY_VIOLATIONS.get() as i64;
        let app = test_app(Arc::new(DenyBridge), false).await;
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sensitive")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap(),
            "hello"
        );
        assert!(metrics::SECURITY_VIOLATIONS.get() as i64 > before);
    }

    #[tokio::test]
    async fn allow_decision_passes_through() {
        let app = test_app(Arc::new(AllowBridge), false).await;
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sensitive")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn enforce_on_sensitive_blocks_denied_request() {
        let app = test_app(Arc::new(DenyBridge), true).await;
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sensitive")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn is_sensitive_classification() {
        // mutating verbs -> sensitive regardless of path
        assert!(is_sensitive("POST", "/query"));
        assert!(is_sensitive("PUT", "/config"));
        assert!(is_sensitive("DELETE", "/sources/x"));
        assert!(is_sensitive("POST", "/ingest/file"));
        assert!(is_sensitive("POST", "/bridge/ai/query"));
        assert!(is_sensitive("POST", "/task/submit"));
        assert!(is_sensitive("POST", "/workflow/run"));
        assert!(is_sensitive("POST", "/sync/upload"));
        // GET reads of secrets/logs/files -> sensitive
        assert!(is_sensitive("GET", "/keys"));
        assert!(is_sensitive("GET", "/audit/log"));
        assert!(is_sensitive("GET", "/sync/download/f.bin"));
        assert!(is_sensitive("GET", "/dev/logs"));
        // GET to write-only prefixes is NOT sensitive (re-scope clears safe bucket)
        assert!(!is_sensitive("GET", "/bridge/ai/models"));
        assert!(!is_sensitive("GET", "/ingest/file"));
        assert!(!is_sensitive("GET", "/upload"));
        assert!(!is_sensitive("GET", "/workflow/run"));
        // non-sensitive GETs
        assert!(!is_sensitive("GET", "/panel"));
        assert!(!is_sensitive("GET", "/status"));
        assert!(!is_sensitive("GET", "/health"));
        assert!(!is_sensitive("GET", "/metrics"));
        assert!(!is_sensitive("GET", "/"));
    }

    #[test]
    fn identity_map() {
        assert_eq!(access_role("nrupalakolkar@gmail.com", None), "admin");
        assert_eq!(access_role("", Some("abc.def.token")), "analyst");
        assert_eq!(access_role("", None), "unknown");
        assert_eq!(access_role("other@example.com", None), "unknown");
        assert_eq!(
            access_role("nrupalakolkar@gmail.com", Some("abc.def")),
            "admin"
        );
    }

    /// A SecurityBridge that records the last (identity, role) it was asked to
    /// authorize and always denies, so tests can assert what identity opa_gate
    /// derived (verified-JWT vs header).
    struct RecordingBridge(Mutex<Option<(String, String)>>);
    impl RecordingBridge {
        fn new() -> Self {
            RecordingBridge(Mutex::new(None))
        }
        fn take(&self) -> (String, String) {
            self.0.lock().unwrap().clone().unwrap_or_default()
        }
    }
    #[async_trait::async_trait]
    impl AetherisBridge for RecordingBridge {
        fn name(&self) -> &str {
            "record"
        }
        async fn health_check(&self) -> bool {
            true
        }
    }
    #[async_trait::async_trait]
    impl SecurityBridge for RecordingBridge {
        async fn authorize(&self, input: &AuthzInput) -> bool {
            *self.0.lock().unwrap() = Some((input.identity.clone(), input.role.clone()));
            false
        }
        async fn authorize_agent(&self, _input: &AuthzInput) -> bool {
            false
        }
        fn enforcing(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn enforce_unverified_sensitive_degrades_to_unknown() {
        // CF_JWT_VERIFY=1 + no/missing assertion on a sensitive route: identity
        // degrades to unknown, role unknown -> denied. The plaintext header must NOT
        // be trusted as admin (closes the spoof gap).
        let rec = Arc::new(RecordingBridge::new());
        let app = test_app_cf(rec.clone() as Arc<dyn SecurityBridge>, true, true).await;
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sensitive")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        let (identity, role) = rec.take();
        assert_eq!(identity, "", "spoofed/unverified identity must not be used");
        assert_eq!(role, "unknown");
    }

    #[tokio::test]
    async fn shadow_keeps_header_identity_on_sensitive() {
        // CF_JWT_VERIFY=0 (shadow): identity still from the plaintext header.
        let rec = Arc::new(RecordingBridge::new());
        let app = test_app_cf(rec.clone() as Arc<dyn SecurityBridge>, false, false).await;
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sensitive")
                    .header(
                        "Cf-Access-Authenticated-User-Email",
                        "nrupalakolkar@gmail.com",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK); // opa_enforce=false -> passes through
        let (identity, role) = rec.take();
        assert_eq!(identity, "nrupalakolkar@gmail.com");
        assert_eq!(role, "admin");
    }

    #[test]
    fn vault_upload_path_rejects_traversal() {
        let vault = std::path::Path::new("/data/vault");
        // Safe plain basenames pass.
        assert_eq!(
            vault_upload_path(vault, "report.txt"),
            Some(vault.join("report.txt"))
        );
        assert_eq!(
            vault_upload_path(vault, "a.b-c_d.png"),
            Some(vault.join("a.b-c_d.png"))
        );
        // Traversal / absolute / dot / separator names are rejected.
        assert!(vault_upload_path(vault, "../etc/passwd").is_none());
        assert!(vault_upload_path(vault, "a/../../etc/passwd").is_none());
        assert!(vault_upload_path(vault, "/etc/passwd").is_none());
        assert!(vault_upload_path(vault, "..").is_none());
        assert!(vault_upload_path(vault, ".").is_none());
        assert!(vault_upload_path(vault, "").is_none());
        assert!(vault_upload_path(vault, "sub\\file.exe").is_none());
        assert!(vault_upload_path(vault, "name..txt").is_none());
    }
}
