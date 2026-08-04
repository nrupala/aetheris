use crate::bridge::{AetherisBridge, AuthzInput, ModelBridge, SecurityBridge};
use async_trait::async_trait;

/// Role derived from the Cloudflare Access identity (see `identity_to_role`).
#[allow(dead_code)] // not wired to a route until Phase 3; unit-tested
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaRole {
    Admin,
    Analyst,
    Unknown,
}

impl OpaRole {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            OpaRole::Admin => "admin",
            OpaRole::Analyst => "analyst",
            OpaRole::Unknown => "unknown",
        }
    }
}

/// Pure, unit-tested mapping from Cloudflare Access identity headers to an
/// OPA role. Not wired to any route yet (Phase 3 wiring).
///
/// * `authenticated_user_email`: value of the `Cf-Access-Authenticated-User-Email`
///   header (present for human users behind Cloudflare Access).
/// * `service_token_client_id`: value of the `Cf-Access-Client-Id` header
///   (present for service-token automation).
#[allow(dead_code)] // not wired to a route until Phase 3; unit-tested
pub fn identity_to_role(
    authenticated_user_email: Option<&str>,
    service_token_client_id: Option<&str>,
) -> OpaRole {
    if authenticated_user_email == Some("nrupalakolkar@gmail.com") {
        OpaRole::Admin
    } else if service_token_client_id.is_some() {
        OpaRole::Analyst
    } else {
        OpaRole::Unknown
    }
}

pub struct OllamaBridge {
    pub url: String,
    pub default_model: String,
    pub embed_fallback_models: Vec<String>,
}

impl OllamaBridge {
    pub fn new(url: String) -> Self {
        Self {
            url,
            default_model: "qwen3:8b".to_string(),
            embed_fallback_models: vec!["nomic-embed-text".to_string()],
        }
    }

    #[allow(dead_code)]
    pub fn with_model(url: String, default_model: String) -> Self {
        Self {
            url,
            default_model,
            embed_fallback_models: vec!["nomic-embed-text".to_string()],
        }
    }
}

#[async_trait]
impl AetherisBridge for OllamaBridge {
    fn name(&self) -> &str {
        "ollama"
    }

    async fn health_check(&self) -> bool {
        crate::util::http_client()
            .get(format!("{}/v1/models", self.url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

impl OllamaBridge {
    pub async fn embed_with_model(&self, content: &str, model: &str) -> Result<Vec<f32>, String> {
        let payload = serde_json::json!({
            "model": model,
            "prompt": content
        });
        let res = crate::util::http_client()
            .post(format!("{}/api/embeddings", self.url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Embedding request failed for {}: {}", model, e))?;
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse embedding response: {}", e))?;
        body["embedding"]
            .as_array()
            .ok_or_else(|| format!("No embedding in response from {}", model))?
            .iter()
            .map(|v| {
                v.as_f64()
                    .map(|f| f as f32)
                    .ok_or_else(|| "Non-float in embedding".to_string())
            })
            .collect()
    }
}

#[async_trait]
impl ModelBridge for OllamaBridge {
    async fn embed(&self, content: &str) -> Result<Vec<f32>, String> {
        let models = &self.embed_fallback_models;
        let mut last_err = String::new();
        for model in models {
            match self.embed_with_model(content, model).await {
                Ok(emb) => return Ok(emb),
                Err(e) => {
                    last_err = e;
                    println!("embed model '{}' unavailable, trying next", model);
                }
            }
        }
        Err(format!("All embed models failed: {}", last_err))
    }

    async fn embed_and_index(&self, content: &str, file_id: &str) -> Result<(), String> {
        let _embedding = self.embed(content).await?;
        println!(
            "Ollama Bridge: Indexed {} ({} dims)",
            file_id,
            _embedding.len()
        );
        Ok(())
    }

    async fn rerank(
        &self,
        query: &str,
        documents: Vec<String>,
        model: &str,
    ) -> Result<Vec<f64>, String> {
        if documents.is_empty() {
            return Ok(vec![]);
        }
        let payload = serde_json::json!({
            "model": model,
            "query": query,
            "documents": documents
        });
        let res = crate::util::http_client()
            .post(format!("{}/api/rerank", self.url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Rerank request failed: {}", e))?;
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse rerank response: {}", e))?;
        body["results"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| r["relevance_score"].as_f64())
                    .collect()
            })
            .ok_or_else(|| "No results in rerank response".to_string())
    }

    async fn list_models(&self) -> Result<Vec<String>, String> {
        let res = crate::util::http_client()
            .get(format!("{}/v1/models", self.url))
            .send()
            .await
            .map_err(|e| format!("Failed to list models: {}", e))?;
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse models response: {}", e))?;
        let models = body["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }

    async fn query(&self, prompt: &str, model: &str) -> Result<String, String> {
        let model = if model.is_empty() {
            &self.default_model
        } else {
            model
        };
        let timeout = crate::util::model_timeout(model);
        let payload = serde_json::json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "temperature": 0.1,
            "stream": false
        });
        let res = crate::util::http_client_with_timeout(timeout)
            .post(format!("{}/v1/chat/completions", self.url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("AI request failed: {}", e))?;
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        let choice = body["choices"].as_array().and_then(|arr| arr.first());
        let msg = choice
            .and_then(|c| c.get("message"))
            .unwrap_or(&serde_json::Value::Null);
        msg["content"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| msg["reasoning_content"].as_str().filter(|s| !s.is_empty()))
            .map(|s| s.to_string())
            .ok_or_else(|| "No content in response".to_string())
    }

    async fn query_with_timeout(
        &self,
        prompt: &str,
        model: &str,
        timeout_secs: u64,
        max_tokens: Option<u32>,
    ) -> Result<String, String> {
        let model = if model.is_empty() {
            &self.default_model
        } else {
            model
        };
        let timeout = std::time::Duration::from_secs(timeout_secs);
        let mut payload = serde_json::json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "temperature": 0.1,
            "stream": false
        });
        if let Some(max_tokens) = max_tokens {
            payload["max_tokens"] = serde_json::json!(max_tokens);
        }
        let res = crate::util::http_client_with_timeout(timeout)
            .post(format!("{}/v1/chat/completions", self.url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("AI request failed: {}", e))?;
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        let choice = body["choices"].as_array().and_then(|arr| arr.first());
        let msg = choice
            .and_then(|c| c.get("message"))
            .unwrap_or(&serde_json::Value::Null);
        msg["content"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| msg["reasoning_content"].as_str().filter(|s| !s.is_empty()))
            .map(|s| s.to_string())
            .ok_or_else(|| "No content in response".to_string())
    }
}

pub struct OpaBridge {
    pub url: String,
    pub fail_open: bool,
    pub enforce: bool,
}

impl OpaBridge {
    pub fn new(url: String, fail_open: bool, enforce: bool) -> Self {
        Self {
            url,
            fail_open,
            enforce,
        }
    }

    /// POST `{"input":{identity,role,method,path,action}}` to the given OPA
    /// decision path and parse the boolean `result` (deny on absent/non-bool).
    async fn eval(&self, decision_path: &str, input: &AuthzInput) -> bool {
        let payload = serde_json::json!({
            "input": {
                "identity": input.identity,
                "role": input.role,
                "method": input.method,
                "path": input.path,
                "action": input.action,
            }
        });
        let client = crate::util::http_client();
        let res = client
            .post(format!("{}/{}", self.url, decision_path))
            .json(&payload)
            .send()
            .await;
        match res {
            Ok(r) => {
                if let Ok(body) = r.json::<serde_json::Value>().await {
                    return body["result"].as_bool().unwrap_or(false);
                }
                false
            }
            Err(e) => {
                if self.fail_open {
                    log::warn!("OPA authz unreachable ({}); failing open", e);
                    crate::metrics::SECURITY_VIOLATIONS.inc();
                    return true;
                }
                false
            }
        }
    }
}

#[async_trait]
impl AetherisBridge for OpaBridge {
    fn name(&self) -> &str {
        "opa"
    }

    async fn health_check(&self) -> bool {
        crate::util::http_client()
            .get(format!("{}/health", self.url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl SecurityBridge for OpaBridge {
    async fn authorize(&self, input: &AuthzInput) -> bool {
        self.eval("v1/data/aetheris/authz/allow", input).await
    }

    async fn authorize_agent(&self, input: &AuthzInput) -> bool {
        self.eval("v1/data/aetheris/agents/allow", input).await
    }

    fn enforcing(&self) -> bool {
        self.enforce
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::AuthzInput;
    use axum::extract::State;
    use axum::routing::post;
    use axum::{Json, Router};

    fn admin_input() -> AuthzInput {
        AuthzInput {
            identity: "user@example.com".to_string(),
            role: "admin".to_string(),
            method: "POST".to_string(),
            path: "/v1/read".to_string(),
            action: "read".to_string(),
        }
    }

    fn input(role: &str, method: &str) -> AuthzInput {
        AuthzInput {
            identity: "user@example.com".to_string(),
            role: role.to_string(),
            method: method.to_string(),
            path: "/v1/read".to_string(),
            action: "read".to_string(),
        }
    }

    /// Start a minimal OPA endpoint that mirrors `config/policy/aetheris.authz.rego`:
    /// admin always allowed; analyst allowed only on GET.
    async fn start_opa() -> String {
        async fn decision(
            State(_): State<()>,
            Json(body): Json<serde_json::Value>,
        ) -> Json<serde_json::Value> {
            let role = body["input"]["role"].as_str().unwrap_or("");
            let method = body["input"]["method"].as_str().unwrap_or("");
            let allowed = role == "admin" || (role == "analyst" && method == "GET");
            Json(serde_json::json!({ "result": allowed }))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route("/v1/data/aetheris/authz/allow", post(decision));
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{}", addr)
    }

    /// Variant that always rejects with an explicit `{"result": false}`.
    async fn start_opa_always_deny() -> String {
        async fn deny() -> Json<serde_json::Value> {
            Json(serde_json::json!({ "result": false }))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route("/v1/data/aetheris/authz/allow", post(deny));
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{}", addr)
    }

    #[tokio::test]
    async fn allow_admin() {
        let url = start_opa().await;
        let bridge = OpaBridge::new(url, true, false);
        assert!(bridge.authorize(&admin_input()).await);
    }

    #[tokio::test]
    async fn deny_unknown() {
        let url = start_opa().await;
        let bridge = OpaBridge::new(url, true, false);
        assert!(!bridge.authorize(&input("unknown", "GET")).await);
    }

    #[tokio::test]
    async fn analyst_get_allowed() {
        let url = start_opa().await;
        let bridge = OpaBridge::new(url, true, false);
        assert!(bridge.authorize(&input("analyst", "GET")).await);
    }

    #[tokio::test]
    async fn analyst_post_denied() {
        let url = start_opa().await;
        let bridge = OpaBridge::new(url, true, false);
        assert!(!bridge.authorize(&input("analyst", "POST")).await);
    }

    #[tokio::test]
    async fn explicit_result_false_denies() {
        let url = start_opa_always_deny().await;
        let bridge = OpaBridge::new(url, true, false);
        assert!(!bridge.authorize(&admin_input()).await);
    }

    #[tokio::test]
    async fn unreachable_fails_open() {
        let bridge = OpaBridge::new("http://127.0.0.1:1".to_string(), true, false);
        assert!(bridge.authorize(&admin_input()).await);
    }

    #[tokio::test]
    async fn unreachable_fails_closed() {
        let bridge = OpaBridge::new("http://127.0.0.1:1".to_string(), false, false);
        assert!(!bridge.authorize(&admin_input()).await);
    }

    #[test]
    fn identity_to_role_admin_email() {
        assert_eq!(
            identity_to_role(Some("nrupalakolkar@gmail.com"), None),
            OpaRole::Admin
        );
    }

    #[test]
    fn identity_to_role_service_token_is_analyst() {
        assert_eq!(
            identity_to_role(None, Some("abc.def.service-token")),
            OpaRole::Analyst
        );
    }

    #[test]
    fn identity_to_role_nothing_is_unknown() {
        assert_eq!(identity_to_role(None, None), OpaRole::Unknown);
    }

    #[test]
    fn identity_to_role_admin_email_beats_token() {
        assert_eq!(
            identity_to_role(Some("nrupalakolkar@gmail.com"), Some("abc.def")),
            OpaRole::Admin
        );
    }

    #[test]
    fn identity_to_role_wrong_email_unknown() {
        assert_eq!(
            identity_to_role(Some("attacker@example.com"), None),
            OpaRole::Unknown
        );
    }

    #[test]
    fn opa_role_as_str() {
        assert_eq!(OpaRole::Admin.as_str(), "admin");
        assert_eq!(OpaRole::Analyst.as_str(), "analyst");
        assert_eq!(OpaRole::Unknown.as_str(), "unknown");
    }

    /// Mirror of config/policy/aetheris.agents.rego. Serves ONLY the agents/allow
    /// path, so a wrong decision path (authz) yields 404 -> result false.
    async fn start_agent_opa() -> String {
        async fn agent_decision(
            State(_): State<()>,
            Json(body): Json<serde_json::Value>,
        ) -> Json<serde_json::Value> {
            let role = body["input"]["role"].as_str().unwrap_or("");
            let action = body["input"]["action"].as_str().unwrap_or("");
            let allowed = match role {
                "researcher" => matches!(
                    action,
                    "query" | "read" | "extract_entities" | "list_sources"
                ),
                "coder" => matches!(
                    action,
                    "write" | "read" | "execute_readonly" | "list_directory"
                ),
                "reviewer" => matches!(action, "read" | "evaluate" | "query_kg" | "list_sources"),
                "planner" => matches!(
                    action,
                    "read" | "query" | "query_kg" | "list_agents" | "coordinate"
                ),
                _ => false,
            };
            Json(serde_json::json!({ "result": allowed }))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route("/v1/data/aetheris/agents/allow", post(agent_decision));
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{}", addr)
    }

    fn agent_input(role: &str, action: &str) -> AuthzInput {
        AuthzInput {
            identity: String::new(),
            role: role.to_string(),
            method: String::new(),
            path: String::new(),
            action: action.to_string(),
        }
    }

    #[tokio::test]
    async fn authorize_agent_allowed_role_actions_pass() {
        let url = start_agent_opa().await;
        let bridge = OpaBridge::new(url, false, false);
        assert!(
            bridge
                .authorize_agent(&agent_input("researcher", "query"))
                .await
        );
        assert!(bridge.authorize_agent(&agent_input("coder", "write")).await);
        assert!(
            bridge
                .authorize_agent(&agent_input("reviewer", "evaluate"))
                .await
        );
        assert!(
            bridge
                .authorize_agent(&agent_input("planner", "coordinate"))
                .await
        );
    }

    #[tokio::test]
    async fn authorize_agent_unknown_role_or_action_denies() {
        let url = start_agent_opa().await;
        let bridge = OpaBridge::new(url, false, false);
        // known role, action not in its allowlist
        assert!(
            !bridge
                .authorize_agent(&agent_input("researcher", "write"))
                .await
        );
        assert!(!bridge.authorize_agent(&agent_input("coder", "query")).await);
        // unknown role
        assert!(!bridge.authorize_agent(&agent_input("admin", "read")).await);
        // empty role/action
        assert!(!bridge.authorize_agent(&agent_input("", "")).await);
    }

    #[tokio::test]
    async fn authorize_agent_hits_agents_path_not_authz() {
        // The server only serves agents/allow; if authorize_agent posted to
        // authz/allow it would 404 -> false, so a true here proves the right path.
        let url = start_agent_opa().await;
        let bridge = OpaBridge::new(url, false, false);
        assert!(
            bridge
                .authorize_agent(&agent_input("researcher", "query"))
                .await
        );
    }
}
