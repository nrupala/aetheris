use crate::bridge::{AetherisBridge, ModelBridge, SecurityBridge};
use async_trait::async_trait;

pub struct OllamaBridge {
    pub url: String,
    pub default_model: String,
}

impl OllamaBridge {
    pub fn new(url: String) -> Self {
        Self {
            url,
            default_model: "qwen3:8b".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn with_model(url: String, default_model: String) -> Self {
        Self { url, default_model }
    }
}

#[async_trait]
impl AetherisBridge for OllamaBridge {
    fn name(&self) -> &str {
        "ollama"
    }

    async fn health_check(&self) -> bool {
        reqwest::Client::new()
            .get(&format!("{}/v1/models", self.url))
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
        let res = reqwest::Client::new()
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
        let models = ["nomic-embed-text", "deepseek-r1:8b", "qwen3:8b"];
        let mut last_err = String::new();
        for model in &models {
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
        let res = reqwest::Client::new()
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
        let res = reqwest::Client::new()
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
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        let msg = &body["choices"][0]["message"];
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
}

impl OpaBridge {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[async_trait]
impl AetherisBridge for OpaBridge {
    fn name(&self) -> &str {
        "opa"
    }

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
            "input": { "peer_id": peer_id, "action": action }
        });
        let client = reqwest::Client::new();
        let res = client
            .post(&format!("{}/v1/data/aetheris/authz/allow", self.url))
            .json(&payload)
            .send()
            .await;
        match res {
            Ok(r) => {
                if let Ok(body) = r.json::<serde_json::Value>().await {
                    return body.get("result").is_some();
                }
                false
            }
            Err(_) => false,
        }
    }
}
