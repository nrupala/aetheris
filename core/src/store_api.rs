//! Persistence-spine HTTP API (Track 2 wiring).
//!
//! Exposes the `store::Store` surfaces (tasks, conversations + messages,
//! namespaced memory, the versioned skill registry, and security watcher bans)
//! as REST endpoints. Read paths are plain GETs; creat/update/delete paths are
//! mutating verbs, so the `opa_gate` middleware treats them as sensitive under
//! `OPA_ENFORCE` just like every other write route in the core.
//!
//! Reasoning: the store module carries the complete persistence API. Exposing
//! every surface here (rather than a hand-picked subset) means no method is
//! left as dead code, and clients/UI/agents can persist exactly what the
//! spine was designed for.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::store::{self, Store};
use crate::AppState;

fn store_or_503() -> StatusCode {
    StatusCode::SERVICE_UNAVAILABLE
}

#[derive(Deserialize)]
struct CreateTaskBody {
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default = "default_agent")]
    agent_id: String,
    #[serde(default)]
    role: String,
    #[serde(default = "default_status")]
    status: String,
}

fn default_agent() -> String {
    "manual".to_string()
}
fn default_status() -> String {
    "pending".to_string()
}

#[derive(Deserialize)]
struct UpdateTaskBody {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct MessageBody {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct MemoryBody {
    content: String,
    #[serde(default = "default_kind")]
    kind: String,
}

fn default_kind() -> String {
    "note".to_string()
}

#[derive(Deserialize)]
struct SkillBody {
    name: String,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_def")]
    definition_json: String,
    #[serde(default = "default_source")]
    source: String,
}

fn default_version() -> String {
    "1.0.0".to_string()
}
fn default_def() -> String {
    "{}".to_string()
}
fn default_source() -> String {
    "local".to_string()
}

#[derive(Deserialize)]
struct BanBody {
    failures: i64,
    last_seen_ms: i64,
    banned_until_ms: i64,
}

/// Persistence spine REST API (mounted under `/store/*`).
pub fn store_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/store/tasks", get(list_tasks).post(create_task))
        .route("/store/tasks/:id", get(get_task).post(update_task))
        .route("/store/conversations", get(list_conversations))
        .route(
            "/store/conversations/:id",
            get(get_conversation).post(create_conversation),
        )
        .route(
            "/store/conversations/:id/messages",
            get(list_messages).post(append_message),
        )
        .route("/store/memory/:namespace", get(list_memory))
        .route(
            "/store/memory/:namespace/:key",
            get(get_memory).put(set_memory).delete(delete_memory),
        )
        .route("/store/skills", get(list_skills))
        .route("/store/skills/:id", get(get_skill).put(put_skill))
        .route("/store/bans", get(list_banned))
        .route("/store/bans/:peer_id", get(get_peer).post(upsert_peer))
}

fn state_store(state: &Arc<AppState>) -> Option<Store> {
    state.store.clone()
}

async fn list_tasks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<store::TaskRow>>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.list_tasks(500)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn create_task(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateTaskBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    let id = s
        .create_task(
            body.request_id.as_deref(),
            &body.agent_id,
            &body.role,
            &body.status,
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "id": id, "status": "created" })))
}

async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<store::TaskRow>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.get_task(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn update_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateTaskBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    if s.get_task(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .is_none()
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let status = body.status.as_deref().unwrap_or("pending");
    s.update_task(id, status, body.output.as_deref(), body.error.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "id": id, "status": status })))
}

async fn list_conversations(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<store::ConversationRow>>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.list_conversations(500)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_conversation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<store::ConversationRow>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.get_conversation(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn create_conversation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<CreateConversationBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.create_conversation(&id, &body.title, &body.user_email)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "id": id, "status": "created" })))
}

#[derive(Deserialize)]
struct CreateConversationBody {
    title: String,
    #[serde(default = "default_email")]
    user_email: String,
}

fn default_email() -> String {
    "unknown".to_string()
}

async fn list_messages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<store::MessageRow>>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.list_messages(&id, 500)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn append_message(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<MessageBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    let msg_id = s
        .append_message(&id, &body.role, &body.content)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        serde_json::json!({ "id": msg_id, "status": "appended" }),
    ))
}

async fn list_memory(
    State(state): State<Arc<AppState>>,
    Path(namespace): Path<String>,
) -> Result<Json<Vec<MemoryEntry>>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    let rows = s
        .list_memory(&namespace)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        rows.into_iter()
            .map(|(key, content)| MemoryEntry {
                namespace: namespace.clone(),
                key,
                content,
            })
            .collect(),
    ))
}

async fn get_memory(
    State(state): State<Arc<AppState>>,
    Path((namespace, key)): Path<(String, String)>,
) -> Result<Json<MemoryEntry>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    let content = s
        .get_memory(&namespace, &key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(MemoryEntry {
        namespace,
        key,
        content,
    }))
}

async fn set_memory(
    State(state): State<Arc<AppState>>,
    Path((namespace, key)): Path<(String, String)>,
    Json(body): Json<MemoryBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.set_memory(&namespace, &key, &body.content, &body.kind)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        serde_json::json!({ "namespace": namespace, "key": key, "status": "set" }),
    ))
}

async fn delete_memory(
    State(state): State<Arc<AppState>>,
    Path((namespace, key)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.delete_memory(&namespace, &key)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        serde_json::json!({ "namespace": namespace, "key": key, "status": "deleted" }),
    ))
}

#[derive(serde::Serialize)]
struct MemoryEntry {
    namespace: String,
    key: String,
    content: String,
}

async fn list_skills(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<store::SkillRow>>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.list_skills()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_skill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<store::SkillRow>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.get_skill(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn put_skill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SkillBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.put_skill(
        &id,
        &body.name,
        &body.version,
        &body.description,
        &body.definition_json,
        &body.source,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "id": id, "status": "set" })))
}

async fn list_banned(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<store::BanRow>>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    s.list_banned(now_ms)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_peer(
    State(state): State<Arc<AppState>>,
    Path(peer_id): Path<String>,
) -> Result<Json<store::BanRow>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.get_peer(&peer_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn upsert_peer(
    State(state): State<Arc<AppState>>,
    Path(peer_id): Path<String>,
    Json(body): Json<BanBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let s = state_store(&state).ok_or_else(store_or_503)?;
    s.upsert_peer(
        &peer_id,
        body.failures,
        body.last_seen_ms,
        body.banned_until_ms,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        serde_json::json!({ "peer_id": peer_id, "status": "set" }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{ModelBridge, SecurityBridge};
    use crate::implementation::{OllamaBridge, OpaBridge};
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state(store: store::Store) -> Arc<AppState> {
        let tmp = std::env::temp_dir().join(format!("store_api_{}.db", std::process::id()));
        let vault_dir = std::env::temp_dir().join("aetheris_store_api_vault");
        std::fs::create_dir_all(&vault_dir).ok();
        let wal_arc: Arc<std::sync::Mutex<()>> = Arc::new(std::sync::Mutex::new(()));
        let _ = &wal_arc;
        let wal_path = vault_dir.join("wal");
        let ai_url = "http://127.0.0.1:1".to_string();
        let model_bridge: Arc<dyn ModelBridge> = Arc::new(OllamaBridge::new(ai_url.clone()));
        let security_bridge: Arc<dyn SecurityBridge> = Arc::new(OpaBridge::new(
            "http://127.0.0.1:1".to_string(),
            true,
            false,
        ));
        let guardian = Arc::new(crate::Guardian::new(
            model_bridge.clone(),
            None,
            None,
            Arc::new(std::sync::Mutex::new(
                crate::wal::WriteAheadLog::new(&wal_path.to_string_lossy()).expect("temp WAL"),
            )),
            vault_dir.clone(),
        ));
        let wal2_path = vault_dir.join("wal2");
        let _ = tmp;
        Arc::new(AppState {
            vault_path: vault_dir.clone(),
            web_root: std::env::temp_dir(),
            security_watcher: Arc::new(crate::watcher::SecurityWatcher::new()),
            ai_url,
            opa_url: "http://127.0.0.1:1".to_string(),
            opa_enforce: false,
            cf_jwt: crate::auth::cf_jwt::CfJwtConfig {
                team_domain: "https://nrupal.cloudflareaccess.com".to_string(),
                aud: std::collections::HashSet::new(),
                jwks_path: std::env::temp_dir().join("missing_jwks.json"),
                enabled: false,
            },
            port_registry: serde_json::json!({}),
            dev_logs: std::sync::Mutex::new(Vec::new()),
            wal: Arc::new(std::sync::Mutex::new(
                crate::wal::WriteAheadLog::new(&wal2_path.to_string_lossy()).expect("temp WAL"),
            )),
            model_bridge,
            security_bridge,
            default_model: "test".to_string(),
            a2a_gateway: std::sync::Mutex::new(crate::A2AGateway::new(10)),
            mcp_server: crate::MCPServer::new(),
            agents: std::sync::Mutex::new(Vec::new()),
            orchestrator_proxy: None,
            openai_proxy: crate::proxy::OpenAiProxy::new("http://127.0.0.1:1/v1".to_string()),
            key_manager: Arc::new(crate::fusion::KeyManager::new(&vault_dir)),
            vector_store: None,
            fusion_router: None,
            guardian,
            rag_config: Arc::new(std::sync::Mutex::new(crate::rag::RagConfig::default())),
            knowledge_graph: None,
            store: Some(store),
            start_time: std::time::Instant::now(),
        })
    }

    async fn app(store: store::Store) -> Router {
        store_router().with_state(test_state(store))
    }

    async fn json_body(res: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn task_lifecycle_via_http() {
        let store = store::Store::in_memory().unwrap();
        let app = app(store).await;

        let create = Request::builder()
            .method("POST")
            .uri("/store/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"request_id":"req-1","agent_id":"planner-a","role":"planner","status":"pending"}"#,
            ))
            .unwrap();
        let res = app.clone().oneshot(create).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let created = json_body(res).await;
        let id = created["id"].as_i64().unwrap();

        let list = Request::builder()
            .method("GET")
            .uri("/store/tasks")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(list).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let tasks = json_body(res).await;
        assert_eq!(tasks.as_array().unwrap().len(), 1);

        let update = Request::builder()
            .method("POST")
            .uri(format!("/store/tasks/{id}"))
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"status":"completed","output":"done","error":null}"#,
            ))
            .unwrap();
        let res = app.clone().oneshot(update).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let get = Request::builder()
            .method("GET")
            .uri(format!("/store/tasks/{id}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(get).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let row = json_body(res).await;
        assert_eq!(row["status"], "completed");
        assert_eq!(row["output"], "done");
    }

    #[tokio::test]
    async fn missing_task_is_not_found() {
        let store = store::Store::in_memory().unwrap();
        let app = app(store).await;
        let res = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/store/tasks/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn conversation_messages_roundtrip() {
        let store = store::Store::in_memory().unwrap();
        let app = app(store).await;

        let create = Request::builder()
            .method("POST")
            .uri("/store/conversations/conv-1")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"title":"RAG","user_email":"nrupalakolkar@gmail.com"}"#,
            ))
            .unwrap();
        let res = app.clone().oneshot(create).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let msg = Request::builder()
            .method("POST")
            .uri("/store/conversations/conv-1/messages")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"role":"user","content":"hello"}"#))
            .unwrap();
        let res = app.clone().oneshot(msg).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let list = Request::builder()
            .method("GET")
            .uri("/store/conversations/conv-1/messages")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(list).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let msgs = json_body(res).await;
        assert_eq!(msgs.as_array().unwrap().len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "hello");
    }

    #[tokio::test]
    async fn memory_crud_via_http() {
        let store = store::Store::in_memory().unwrap();
        let app = app(store).await;

        let set = Request::builder()
            .method("PUT")
            .uri("/store/memory/user/name")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"content":"Milo","kind":"note"}"#))
            .unwrap();
        let res = app.clone().oneshot(set).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let get = Request::builder()
            .method("GET")
            .uri("/store/memory/user/name")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(get).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let entry = json_body(res).await;
        assert_eq!(entry["content"], "Milo");
        assert_eq!(entry["key"], "name");

        let list = Request::builder()
            .method("GET")
            .uri("/store/memory/user")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(list).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let rows = json_body(res).await;
        assert_eq!(rows.as_array().unwrap().len(), 1);

        let del = Request::builder()
            .method("DELETE")
            .uri("/store/memory/user/name")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(del).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let get = Request::builder()
            .method("GET")
            .uri("/store/memory/user/name")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(get).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn skills_and_bans_roundtrip() {
        let store = store::Store::in_memory().unwrap();
        let app = app(store).await;

        let put = Request::builder()
            .method("PUT")
            .uri("/store/skills/sk-1")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"name":"rag-query","version":"1.0.0","description":"RAG","definition_json":"{}","source":"local"}"#,
            ))
            .unwrap();
        let res = app.clone().oneshot(put).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let list = Request::builder()
            .method("GET")
            .uri("/store/skills")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(list).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let skills = json_body(res).await;
        assert_eq!(skills.as_array().unwrap().len(), 1);
        assert_eq!(skills[0]["id"], "sk-1");

        let ban = Request::builder()
            .method("POST")
            .uri("/store/bans/peer-1")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"failures":5,"last_seen_ms":1000,"banned_until_ms":2000000000000}"#,
            ))
            .unwrap();
        let res = app.clone().oneshot(ban).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let get = Request::builder()
            .method("GET")
            .uri("/store/bans/peer-1")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(get).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let row = json_body(res).await;
        assert_eq!(row["failures"], 5);

        let active = Request::builder()
            .method("GET")
            .uri("/store/bans")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(active).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bans = json_body(res).await;
        assert_eq!(bans.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn ours_and_theirs_store_isolation() {
        // Each in-memory store is its own namespace: writes in one app are not
        // visible in another.
        let store_a = store::Store::in_memory().unwrap();
        let app_a = app(store_a).await;
        let store_b = store::Store::in_memory().unwrap();
        let app_b = app(store_b).await;

        let set = Request::builder()
            .method("PUT")
            .uri("/store/memory/a/k")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"content":"x","kind":"note"}"#))
            .unwrap();
        let _ = app_a.clone().oneshot(set).await.unwrap();

        let get = Request::builder()
            .method("GET")
            .uri("/store/memory/a/k")
            .body(Body::empty())
            .unwrap();
        let res = app_b.oneshot(get).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
