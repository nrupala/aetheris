use async_trait::async_trait;

#[async_trait]
pub trait AetherisBridge: Send + Sync {
    fn name(&self) -> &str;
    async fn health_check(&self) -> bool;
}

/// The authorization input contract sent to OPA's
/// `POST /v1/data/aetheris/authz/allow`. Mirrors the `input` document in
/// `config/policy/aetheris.authz.rego`.
#[derive(Debug, Clone)]
pub struct AuthzInput {
    pub identity: String,
    pub role: String,
    pub method: String,
    pub path: String,
    pub action: String,
}

#[async_trait]
pub trait SecurityBridge: AetherisBridge {
    async fn authorize(&self, input: &AuthzInput) -> bool;
}

#[async_trait]
#[allow(dead_code)]
pub trait AIBridge: AetherisBridge {
    async fn embed_and_index(&self, content: &str, file_id: &str) -> Result<(), String>;
}

#[async_trait]
pub trait ModelBridge: AetherisBridge {
    async fn query(&self, prompt: &str, model: &str) -> Result<String, String>;
    async fn query_with_timeout(
        &self,
        prompt: &str,
        model: &str,
        _timeout_secs: u64,
        _max_tokens: Option<u32>,
    ) -> Result<String, String> {
        self.query(prompt, model).await
    }
    async fn embed(&self, content: &str) -> Result<Vec<f32>, String>;
    async fn embed_and_index(&self, content: &str, file_id: &str) -> Result<(), String>;
    async fn list_models(&self) -> Result<Vec<String>, String>;
    async fn rerank(
        &self,
        query: &str,
        documents: Vec<String>,
        model: &str,
    ) -> Result<Vec<f64>, String>;
}
