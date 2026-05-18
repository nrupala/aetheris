use async_trait::async_trait;
use crate::bridge::{AIBridge, SecurityBridge, AetherisBridge};

pub struct AIBridge { pub url: String }

#[async_trait]
impl AetherisBridge for AIBridge {
    fn name(&self) -> &str { "lmstudio" }
    async fn health_check(&self) -> bool {
        reqwest::Client::new()
            .get(&format!("{}/v1/models", self.url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl crate::bridge::ModelBridge for AIBridge {
    async fn embed_and_index(&self, content: &str, file_id: &str) -> Result<(), String> {
        println!("AI Bridge: Indexing {} via LMStudio...", file_id);
        
        let payload = serde_json::json!({
            "model": "text-embedding-nomic-embed-text-v1.5",
            "input": content
        });

        reqwest::Client::new()
            .post(format!("{}/v1/embeddings", self.url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn query(&self, prompt: &str, model: &str) -> Result<String, String> {
        let payload = serde_json::json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "temperature": 0.1,
            "stream": false
        });

        let res = reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("AI request failed: {}", e))?;

        let body: serde_json::Value = res.json().await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let msg = &body["choices"][0]["message"];
        msg["content"].as_str().filter(|s| !s.is_empty())
            .or_else(|| msg["reasoning_content"].as_str().filter(|s| !s.is_empty()))
            .map(|s| s.to_string())
            .ok_or_else(|| "No content in response".to_string())
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
