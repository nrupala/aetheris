use crate::bridge::{AetherisBridge, ModelBridge};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub service: String,
    pub key: String,
    pub label: String,
    pub configured_at: String,
    pub enabled: bool,
}

impl ApiKeyEntry {
    fn masked_key(&self) -> String {
        if self.key.len() <= 8 {
            return "********".to_string();
        }
        format!("{}...{}", &self.key[..4], &self.key[self.key.len() - 4..])
    }

    pub fn to_public(&self) -> serde_json::Value {
        serde_json::json!({
            "service": self.service,
            "key": self.masked_key(),
            "label": self.label,
            "configured_at": self.configured_at,
            "enabled": self.enabled,
        })
    }
}

pub struct KeyManager {
    keys: Mutex<HashMap<String, ApiKeyEntry>>,
    store_path: std::path::PathBuf,
}

impl KeyManager {
    pub fn new(vault_path: &std::path::Path) -> Self {
        let store_path = vault_path.join("keys.json");
        let keys = if let Ok(content) = std::fs::read_to_string(&store_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };
        Self {
            keys: Mutex::new(keys),
            store_path,
        }
    }

    pub fn get(&self, service: &str) -> Option<String> {
        let keys = self.keys.lock().unwrap();
        keys.get(service)
            .filter(|e| e.enabled)
            .map(|e| e.key.clone())
    }

    pub fn set(&self, service: &str, key: String, label: String) -> Result<(), String> {
        let mut keys = self.keys.lock().unwrap();
        let entry = ApiKeyEntry {
            service: service.to_string(),
            key,
            label,
            configured_at: format!("{:?}", std::time::SystemTime::now()),
            enabled: true,
        };
        keys.insert(service.to_string(), entry);
        self.save(&keys)
    }

    pub fn toggle(&self, service: &str, enabled: bool) -> Result<(), String> {
        let mut keys = self.keys.lock().unwrap();
        if let Some(entry) = keys.get_mut(service) {
            entry.enabled = enabled;
            self.save(&keys)
        } else {
            Err(format!("Service '{}' not found", service))
        }
    }

    pub fn delete(&self, service: &str) -> Result<(), String> {
        let mut keys = self.keys.lock().unwrap();
        keys.remove(service);
        self.save(&keys)
    }

    pub fn list_public(&self) -> Vec<serde_json::Value> {
        let keys = self.keys.lock().unwrap();
        keys.values().map(|e| e.to_public()).collect()
    }

    fn save(&self, keys: &HashMap<String, ApiKeyEntry>) -> Result<(), String> {
        let json = serde_json::to_string_pretty(keys).map_err(|e| e.to_string())?;
        std::fs::write(&self.store_path, &json).map_err(|e| e.to_string())
    }
}

pub struct OpenRouterBridge {
    pub url: String,
    pub default_model: String,
    pub key_manager: Arc<KeyManager>,
}

impl OpenRouterBridge {
    pub fn new(url: String, default_model: String, key_manager: Arc<KeyManager>) -> Self {
        Self {
            url,
            default_model,
            key_manager,
        }
    }

    fn api_key(&self) -> Option<String> {
        self.key_manager.get("openrouter")
    }
}

#[async_trait]
impl AetherisBridge for OpenRouterBridge {
    fn name(&self) -> &str {
        "openrouter"
    }

    async fn health_check(&self) -> bool {
        self.api_key().is_some()
    }
}

#[async_trait]
impl ModelBridge for OpenRouterBridge {
    async fn query(&self, prompt: &str, model: &str) -> Result<String, String> {
        let api_key = self
            .api_key()
            .ok_or_else(|| "OpenRouter not configured".to_string())?;
        let model = if model.is_empty() {
            &self.default_model
        } else {
            model
        };
        let payload = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.1,
        });
        let client = reqwest::Client::new();
        let res = client
            .post(format!("{}/v1/chat/completions", self.url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("HTTP-Referer", "https://github.com/aetheris")
            .header("X-Title", "Aetheris")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("OpenRouter request failed: {}", e))?;
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse OpenRouter response: {}", e))?;
        body["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No content in OpenRouter response".to_string())
    }

    async fn embed(&self, _content: &str) -> Result<Vec<f32>, String> {
        Err("OpenRouter does not support embeddings".to_string())
    }

    async fn embed_and_index(&self, _content: &str, _file_id: &str) -> Result<(), String> {
        Err("OpenRouter does not support embeddings".to_string())
    }

    async fn rerank(
        &self,
        _query: &str,
        _documents: Vec<String>,
        _model: &str,
    ) -> Result<Vec<f64>, String> {
        Err("OpenRouter does not support reranking".to_string())
    }

    async fn list_models(&self) -> Result<Vec<String>, String> {
        let api_key = self
            .api_key()
            .ok_or_else(|| "OpenRouter not configured".to_string())?;
        let client = reqwest::Client::new();
        let res = client
            .get(format!("{}/v1/models", self.url))
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| format!("OpenRouter list models failed: {}", e))?;
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse OpenRouter models: {}", e))?;
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
}

pub struct ExaSearchBridge {
    pub url: String,
    pub key_manager: Arc<KeyManager>,
}

impl ExaSearchBridge {
    pub fn new(url: String, key_manager: Arc<KeyManager>) -> Self {
        Self { url, key_manager }
    }

    fn api_key(&self) -> Option<String> {
        self.key_manager.get("exasearch")
    }

    pub async fn search(
        &self,
        query: &str,
        num_results: usize,
    ) -> Result<serde_json::Value, String> {
        let api_key = self
            .api_key()
            .ok_or_else(|| "ExaSearch not configured".to_string())?;
        let payload = serde_json::json!({
            "query": query,
            "num_results": num_results,
        });
        let client = reqwest::Client::new();
        let res = client
            .post(format!("{}/v1/search", self.url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("ExaSearch request failed: {}", e))?;
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse ExaSearch response: {}", e))?;
        Ok(body)
    }
}

pub struct FusionRouter {
    pub ollama: Arc<dyn ModelBridge>,
    pub openrouter: Option<Arc<OpenRouterBridge>>,
    pub exasearch: Option<Arc<ExaSearchBridge>>,
    pub key_manager: Arc<KeyManager>,
}

impl Clone for FusionRouter {
    fn clone(&self) -> Self {
        Self {
            ollama: self.ollama.clone(),
            openrouter: self.openrouter.clone(),
            exasearch: self.exasearch.clone(),
            key_manager: self.key_manager.clone(),
        }
    }
}

impl FusionRouter {
    pub fn new(
        ollama: Arc<dyn ModelBridge>,
        openrouter: Option<Arc<OpenRouterBridge>>,
        exasearch: Option<Arc<ExaSearchBridge>>,
        key_manager: Arc<KeyManager>,
    ) -> Self {
        Self {
            ollama,
            openrouter,
            exasearch,
            key_manager,
        }
    }

    pub async fn query_with_fallback(&self, prompt: &str, model: &str) -> Result<String, String> {
        let result = self.ollama.query(prompt, model).await?;
        Ok(result)
    }

    pub async fn smart_query(&self, user_query: &str) -> Result<String, String> {
        let local_result = self.ollama.query(user_query, "qwen2.5:14b").await?;

        if let Some(openrouter) = &self.openrouter {
            if openrouter.api_key().is_some() {
                let refined = format!(
                    "Based on this context: {}\n\nImprove and expand this answer: {}",
                    local_result, user_query
                );
                if let Ok(remote) = openrouter.query(&refined, "openai/gpt-4o-mini").await {
                    return Ok(remote);
                }
            }
        }

        Ok(local_result)
    }

    pub async fn search_and_synthesize(&self, query: &str) -> Result<String, String> {
        if let Some(exasearch) = &self.exasearch {
            if exasearch.api_key().is_some() {
                if let Ok(search_results) = exasearch.search(query, 5).await {
                    let context = serde_json::json!({
                        "query": query,
                        "search_results": search_results,
                    });
                    let synthesis_prompt = format!(
                        "Answer this question based on the provided search results:\n\nQuestion: {}\n\nSearch Results: {}\n\nProvide a comprehensive answer with citations.",
                        query, context
                    );
                    return self.ollama.query(&synthesis_prompt, "qwen2.5:14b").await;
                }
            }
        }
        self.smart_query(query).await
    }
}
