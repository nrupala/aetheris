use async_trait::async_trait;
use std::sync::Arc;
use std::time::SystemTime;

use super::{Agent, AgentResult, AgentRole, AgentState, AgentStatus, BaseAgent};
use crate::bridge::{ModelBridge, SecurityBridge};

pub struct ReviewerAgent {
    base: BaseAgent,
}

impl ReviewerAgent {
    pub fn new(
        model: String,
        system_prompt: String,
        model_bridge: Option<Arc<dyn ModelBridge>>,
        security_bridge: Option<Arc<dyn SecurityBridge>>,
    ) -> Self {
        Self {
            base: BaseAgent::new(
                AgentRole::Reviewer,
                model,
                if system_prompt.is_empty() {
                    "You are a code review agent. Evaluate content for correctness, quality, and security.".to_string()
                } else {
                    system_prompt
                },
                model_bridge,
                security_bridge,
            ),
        }
    }
}

#[async_trait]
impl Agent for ReviewerAgent {
    fn id(&self) -> &str {
        &self.base.id
    }
    fn role(&self) -> AgentRole {
        AgentRole::Reviewer
    }
    fn model(&self) -> &str {
        &self.base.model
    }
    fn state(&self) -> AgentState {
        self.base.state.clone()
    }
    fn tasks_completed(&self) -> u64 {
        self.base.task_history.len() as u64
    }
    fn policy_checks(&self) -> u64 {
        self.base.policies_checked
    }
    fn policy_allowed(&self) -> u64 {
        self.base.policies_allowed
    }

    fn get_status(&self) -> AgentStatus {
        AgentStatus {
            id: self.base.id.clone(),
            role: self.base.role.as_str().to_string(),
            model: self.base.model.clone(),
            state: self.base.state.as_str().to_string(),
            tasks_completed: self.tasks_completed(),
            policy_checks: self.policy_checks(),
            policy_allowed: self.policy_allowed(),
        }
    }

    fn reset(&mut self) {
        self.base.state = AgentState::Idle;
    }

    async fn execute(&mut self, task: &str, context: &serde_json::Value) -> AgentResult {
        let start = SystemTime::now();
        self.base.state = AgentState::Executing;

        if !self.base.check_policy("evaluate", task).await {
            self.base.state = AgentState::Failed;
            return AgentResult {
                agent_id: self.base.id.clone(),
                role: "reviewer".to_string(),
                task: task.to_string(),
                output: String::new(),
                metadata: serde_json::json!({}),
                duration_ms: start.elapsed().unwrap_or_default().as_secs_f64() * 1000.0,
                tokens_used: 0,
                success: false,
                error: Some("Policy denied: evaluate".to_string()),
            };
        }

        let content = context
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let standards_context = context
            .get("rag_answer")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let kg_history = context
            .get("kg_context")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let criteria = context
            .get("criteria")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                vec![
                    "correctness".to_string(),
                    "quality".to_string(),
                    "security".to_string(),
                ]
            });

        let review_prompt = format!(
            "Evaluate the following content against these criteria: {:?}\n\nContent:\n{}\n\nBest practices context:\n{}\n\nPast decisions (KG history):\n{}\n\nProvide a structured review as JSON with fields: score (1-10), criteria_scores (object), strengths (list), issues (list), suggestions (list), verdict (\"approve\"/\"comment\"/\"request_changes\"/\"reject\").",
            criteria, content, standards_context, kg_history
        );

        let review_result = self.base.call_llm(vec![
            serde_json::json!({"role": "system", "content": &self.base.system_prompt}),
            serde_json::json!({"role": "user", "content": &review_prompt}),
        ], 0.1, 4096).await.unwrap_or_else(|e| {
            format!("{{\"score\": 5, \"verdict\": \"pending\", \"error\": \"LLM call failed: {}\"}}", e)
        });

        let review: serde_json::Value = serde_json::from_str(&review_result).unwrap_or_else(|_| {
            serde_json::json!({
                "score": 5,
                "criteria_scores": {},
                "strengths": [],
                "issues": ["Could not parse structured review"],
                "suggestions": [],
                "verdict": "pending",
                "raw_review": review_result,
            })
        });

        let duration_ms = start.elapsed().unwrap_or_default().as_secs_f64() * 1000.0;
        self.base.state = AgentState::Complete;
        self.base
            .task_history
            .push(serde_json::json!({"task": task, "success": true}));

        AgentResult {
            agent_id: self.base.id.clone(),
            role: "reviewer".to_string(),
            task: task.to_string(),
            output: serde_json::to_string_pretty(&review).unwrap_or_default(),
            metadata: review,
            duration_ms,
            tokens_used: 0,
            success: true,
            error: None,
        }
    }
}
