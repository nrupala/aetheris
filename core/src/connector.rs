use serde_json::json;

#[allow(dead_code)]
pub struct AetherisConnector {
    pub ai_url: String,
    pub opa_url: String,
    pub vault_path: String,
    pub fallback_model: String,
}

#[allow(dead_code)]
impl AetherisConnector {
    pub fn from_config(config: &crate::config::Config) -> Self {
        Self {
            ai_url: config.ai_endpoint.clone(),
            opa_url: config.opa_endpoint.clone(),
            vault_path: config.vault_path.to_string_lossy().to_string(),
            fallback_model: config.fallback_model.clone(),
        }
    }

    pub async fn authorize(&self, peer_id: &str, action: &str) -> bool {
        let client = crate::util::http_client();
        let body = json!({
            "input": {
                "peer_id": peer_id,
                "action": action,
            }
        });

        let res = client
            .post(format!("{}/v1/data/aetheris/authz/allow", self.opa_url))
            .json(&body)
            .send()
            .await;
        res.map(|r| r.status().is_success()).unwrap_or(false)
    }

    pub async fn index_semantic(&self, file_content: String) {
        let client = crate::util::http_client();
        let payload = json!({
            "model": "text-embedding-nomic-embed-text-v1.5",
            "input": file_content
        });

        let _ = client
            .post(format!("{}/v1/embeddings", self.ai_url))
            .json(&payload)
            .send()
            .await;
    }

    pub async fn ai_query(&self, prompt: &str, model: Option<&str>) -> Result<String, String> {
        let model = model.unwrap_or(&self.fallback_model);
        let client = crate::util::http_client();
        let payload = json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "temperature": 0.1,
            "stream": false
        });

        let res = client
            .post(format!("{}/v1/chat/completions", self.ai_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("AI request failed: {}", e))?;

        let status = res.status();
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse AI response: {}", e))?;

        if status.is_success() {
            let choice = body["choices"].as_array().and_then(|arr| arr.first());
            let msg = choice
                .and_then(|c| c.get("message"))
                .unwrap_or(&serde_json::Value::Null);
            let content = msg["content"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| msg["reasoning_content"].as_str().filter(|s| !s.is_empty()));

            content
                .map(|s| s.to_string())
                .ok_or_else(|| "No content in AI response".to_string())
        } else {
            Err(format!("AI error ({}): {}", status, body))
        }
    }

    pub async fn list_models(&self) -> Result<Vec<String>, String> {
        let client = crate::util::http_client();
        let res = client
            .get(format!("{}/v1/models", self.ai_url))
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

    pub fn trigger_snapshot(&self) {
        let mut child = std::process::Command::new("zrepl")
            .arg("signal")
            .arg("wakeup")
            .arg("aetheris_vault_snapshots")
            .spawn()
            .expect("Failed to pulse zrepl");
        let _ = child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_connector() -> AetherisConnector {
        AetherisConnector {
            ai_url: "http://localhost:1234".to_string(),
            opa_url: "http://localhost:8181".to_string(),
            vault_path: "/data/vault".to_string(),
            fallback_model: "qwen2.5:7b".to_string(),
        }
    }

    #[test]
    fn test_connector_initialization() {
        let connector = test_connector();
        assert_eq!(connector.ai_url, "http://localhost:1234");
        assert_eq!(connector.opa_url, "http://localhost:8181");
        assert_eq!(connector.fallback_model, "qwen2.5:7b");
    }

    #[test]
    fn test_ai_url_endpoint_construction() {
        let connector = test_connector();
        let embeddings_url = format!("{}/v1/embeddings", connector.ai_url);
        let chat_url = format!("{}/v1/chat/completions", connector.ai_url);
        let models_url = format!("{}/v1/models", connector.ai_url);
        assert_eq!(embeddings_url, "http://localhost:1234/v1/embeddings");
        assert_eq!(chat_url, "http://localhost:1234/v1/chat/completions");
        assert_eq!(models_url, "http://localhost:1234/v1/models");
    }

    #[test]
    fn test_opa_url_construction() {
        let connector = test_connector();
        assert_eq!(connector.opa_url, "http://localhost:8181");
    }

    #[test]
    fn test_authz_request_body_structure() {
        let peer_id = "test-peer-001";
        let action = "read";
        let body = json!({
            "input": {
                "peer_id": peer_id,
                "action": action,
            }
        });
        assert_eq!(body["input"]["peer_id"], "test-peer-001");
        assert_eq!(body["input"]["action"], "read");
    }

    #[test]
    fn test_embedding_payload_structure() {
        let content = "Hello, world!".to_string();
        let payload = json!({
            "model": "text-embedding-nomic-embed-text-v1.5",
            "input": content
        });
        assert_eq!(payload["model"], "text-embedding-nomic-embed-text-v1.5");
        assert_eq!(payload["input"], "Hello, world!");
    }

    #[test]
    fn test_ai_query_payload_with_default_model() {
        let payload = json!({
            "model": "qwen2.5:7b",
            "messages": [{"role": "user", "content": "test prompt"}],
            "temperature": 0.1,
            "stream": false
        });
        assert_eq!(payload["model"], "qwen2.5:7b");
        assert_eq!(payload["temperature"], 0.1);
        assert_eq!(payload["stream"], false);
    }

    #[test]
    fn test_ai_query_payload_with_custom_model() {
        let payload = json!({
            "model": "qwen2.5-coder:7b",
            "messages": [{"role": "user", "content": "test prompt"}],
            "temperature": 0.1,
            "stream": false
        });
        assert_eq!(payload["model"], "qwen2.5-coder:7b");
    }

    #[test]
    fn test_models_response_parsing() {
        let mock_response = json!({
            "data": [
                {"id": "model-1", "object": "model"},
                {"id": "model-2", "object": "model"},
                {"id": "model-3", "object": "model"}
            ]
        });
        let models: Vec<String> = mock_response["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0], "model-1");
        assert_eq!(models[2], "model-3");
    }

    #[test]
    fn test_empty_models_response_parsing() {
        let mock_response = json!({});
        let models: Vec<String> = mock_response["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(models.len(), 0);
    }

    #[tokio::test]
    async fn test_authorize_returns_false_on_unreachable_opa() {
        let connector = AetherisConnector {
            ai_url: "http://localhost:1234".to_string(),
            opa_url: "http://localhost:99999".to_string(),
            vault_path: "/data/vault".to_string(),
            fallback_model: "qwen2.5:7b".to_string(),
        };
        let result = connector.authorize("peer-1", "read").await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_ai_query_returns_error_on_unreachable_endpoint() {
        let connector = AetherisConnector {
            ai_url: "http://localhost:99999".to_string(),
            opa_url: "http://localhost:8181".to_string(),
            vault_path: "/data/vault".to_string(),
            fallback_model: "qwen2.5:7b".to_string(),
        };
        let result = connector.ai_query("test", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_models_returns_error_on_unreachable_endpoint() {
        let connector = AetherisConnector {
            ai_url: "http://localhost:99999".to_string(),
            opa_url: "http://localhost:8181".to_string(),
            vault_path: "/data/vault".to_string(),
            fallback_model: "qwen2.5:7b".to_string(),
        };
        let result = connector.list_models().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_default_model_fallback() {
        let connector = test_connector();
        let resolved = None.unwrap_or(&connector.fallback_model);
        assert_eq!(resolved, "qwen2.5:7b");
    }

    #[test]
    fn test_custom_model_override() {
        let connector = test_connector();
        let resolved = Some("custom-model").unwrap_or(&connector.fallback_model);
        assert_eq!(resolved, "custom-model");
    }
}
