use async_trait::async_trait;
use crate::bridge::{AIBridge, SecurityBridge, AetherisBridge};

pub struct OllamaBridge { pub url: String }

#[async_trait]
impl AetherisBridge for OllamaBridge {
    fn name(&self) -> &str { "ollama" }
    async fn health_check(&self) -> bool {
        reqwest::Client::new()
            .get(&format!("{}/api/tags", self.url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl AIBridge for OllamaBridge {
    async fn embed_and_index(&self, content: &str, file_id: &str) -> Result<(), String> {
        println!("AI Bridge: Indexing {}...", file_id);
        
        let payload = serde_json::json!({
            "model": "nomic-embed-text",
            "prompt": content
        });

        reqwest::Client::new()
            .post(format!("{}/api/embeddings", self.url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}

pub struct OpaBridge { pub url: String }

#[async_trait]
impl AetherisBridge for OpaBridge {
    fn name(&self) -> &str { "opa" }
    async fn health_check(&self) -> bool {
        reqwest::Client::new()
            .get(&format!("{}/health", self.url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl SecurityBridge for OpaBridge {
    async fn authorize(&self, peer_id: &str, action: &str) -> bool {
        let payload = serde_json::json!({
            "input": {
                "peer_id": peer_id,
                "action": action
            }
        });

        reqwest::Client::new()
            .post(&format!("{}/v1/data/aetheris/authz/allow", self.url))
            .json(&payload)
            .send()
            .await
            .map(|r| r.json::<serde_json::Value>().await.map(|v| v.get("result").is_some()).unwrap_or(false))
            .unwrap_or(false)
    }
}
