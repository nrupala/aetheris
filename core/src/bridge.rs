use async_trait::async_trait;

#[async_trait]
pub trait AetherisBridge {
    fn name(&self) -> &str;
    async fn health_check(&self) -> bool;
}

#[async_trait]
pub trait SecurityBridge: AetherisBridge {
    async fn authorize(&self, peer_id: &str, action: &str) -> bool;
}

#[async_trait]
pub trait AIBridge: AetherisBridge {
    async fn embed_and_index(&self, content: &str, file_id: &str) -> Result<(), String>;
}
