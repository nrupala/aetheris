use async_trait::async_trait;

#[async_trait]
pub trait AetherisBridge: Send + Sync {
    fn name(&self) -> &str;
    async fn health_check(&self) -> bool;
}

#[async_trait]
pub trait SecurityBridge: AetherisBridge {
    async fn authorize(&self, peer_id: &str, action: &str) -> bool;
}

#[async_trait]
#[allow(dead_code)]
pub trait AIBridge: AetherisBridge {
    async fn embed_and_index(&self, content: &str, file_id: &str) -> Result<(), String>;
}

#[async_trait]
pub trait ModelBridge: AetherisBridge {
    async fn query(&self, prompt: &str, model: &str) -> Result<String, String>;
    async fn embed(&self, content: &str) -> Result<Vec<f32>, String>;
    async fn embed_and_index(&self, content: &str, file_id: &str) -> Result<(), String>;
    async fn list_models(&self) -> Result<Vec<String>, String>;
}
