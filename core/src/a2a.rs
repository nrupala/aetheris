use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct A2AMessage {
    pub id: String,
    pub conversation_id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: String,
    pub content: serde_json::Value,
    pub timestamp: u64,
    pub policy_approved: bool,
}

pub struct A2AGateway {
    conversations: Mutex<HashMap<String, Vec<A2AMessage>>>,
    message_log: Mutex<Vec<A2AMessage>>,
    ttl_seconds: u64,
}

impl A2AGateway {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            conversations: Mutex::new(HashMap::new()),
            message_log: Mutex::new(Vec::new()),
            ttl_seconds,
        }
    }

    pub fn send(
        &self,
        from_agent: &str,
        to_agent: &str,
        conversation_id: &str,
        message_type: &str,
        content: serde_json::Value,
        approved: bool,
    ) -> String {
        let id = format!("msg_{:x}", {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        });

        let msg = A2AMessage {
            id: id.clone(),
            conversation_id: conversation_id.to_string(),
            from_agent: from_agent.to_string(),
            to_agent: to_agent.to_string(),
            message_type: message_type.to_string(),
            content,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            policy_approved: approved,
        };

        let mut convs = self.conversations.lock().unwrap();
        convs
            .entry(conversation_id.to_string())
            .or_insert_with(Vec::new)
            .push(msg.clone());

        self.message_log.lock().unwrap().push(msg);
        id
    }

    pub fn receive(&self, conversation_id: &str, agent_id: &str) -> Vec<A2AMessage> {
        let convs = self.conversations.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(messages) = convs.get(conversation_id) {
            messages
                .iter()
                .filter(|m| m.to_agent == agent_id && (now - m.timestamp) < self.ttl_seconds)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_message_log(&self, limit: usize) -> Vec<serde_json::Value> {
        let log = self.message_log.lock().unwrap();
        log.iter()
            .rev()
            .take(limit)
            .map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "conversation_id": m.conversation_id,
                    "from_agent": m.from_agent,
                    "to_agent": m.to_agent,
                    "message_type": m.message_type,
                    "timestamp": m.timestamp,
                    "approved": m.policy_approved,
                })
            })
            .collect()
    }

    pub fn cleanup_expired(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut convs = self.conversations.lock().unwrap();
        let mut removed = 0u64;

        for messages in convs.values_mut() {
            let before = messages.len();
            messages.retain(|m| (now - m.timestamp) < self.ttl_seconds);
            removed += (before - messages.len()) as u64;
        }

        convs.retain(|_, msgs| !msgs.is_empty());
        removed
    }
}
