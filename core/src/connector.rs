use serde_json::json;

pub struct AetherisConnector {
    pub ai_url: String,
    pub opa_url: String,
    pub vault_path: String,
}

impl AetherisConnector {
    pub async fn authorize(&self, peer_id: &str, action: &str) -> bool {
        let client = reqwest::Client::new();
        let body = json!({
            "input": {
                "peer_id": peer_id,
                "action": action,
            }
        });
        
        let res = client.post(&self.opa_url).json(&body).send().await;
        res.map(|r| r.status().is_success()).unwrap_or(false)
    }

    pub async fn index_semantic(&self, file_content: String) {
        let client = reqwest::Client::new();
        let payload = json!({
            "model": "nomic-embed-text",
            "prompt": file_content
        });

        let _ = client.post(format!("{}/api/embeddings", self.ai_url))
            .json(&payload)
            .send()
            .await;
    }

    pub fn trigger_snapshot(&self) {
        std::process::Command::new("zrepl")
            .arg("signal")
            .arg("wakeup")
            .arg("aetheris_vault_snapshots")
            .spawn()
            .expect("Failed to pulse zrepl");
    }
}
