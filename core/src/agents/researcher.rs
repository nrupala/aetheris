use std::sync::Arc;
use async_trait::async_trait;
use std::time::SystemTime;

use crate::bridge::{ModelBridge, SecurityBridge};
use super::{Agent, AgentResult, AgentRole, AgentState, AgentStatus, BaseAgent};

pub struct ResearcherAgent {
    base: BaseAgent,
}

impl ResearcherAgent {
    pub fn new(model: String, system_prompt: String,
               model_bridge: Option<Arc<dyn ModelBridge>>,
               security_bridge: Option<Arc<dyn SecurityBridge>>) -> Self {
        Self {
            base: BaseAgent::new(
                AgentRole::Researcher, model,
                if system_prompt.is_empty() { "You are a research agent. Gather information and synthesize findings.".to_string() } else { system_prompt },
                model_bridge, security_bridge,
            ),
        }
    }
}

#[async_trait]
impl Agent for ResearcherAgent {
    fn id(&self) -> &str { &self.base.id }
    fn role(&self) -> AgentRole { AgentRole::Researcher }
    fn model(&self) -> &str { &self.base.model }
    fn state(&self) -> AgentState { self.base.state.clone() }
    fn tasks_completed(&self) -> u64 { self.base.task_history.len() as u64 }
    fn policy_checks(&self) -> u64 { self.base.policies_checked }
    fn policy_allowed(&self) -> u64 { self.base.policies_allowed }

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

        if !self.base.check_policy("query", task).await {
            self.base.state = AgentState::Failed;
            return AgentResult {
                agent_id: self.base.id.clone(), role: "researcher".to_string(),
                task: task.to_string(), output: String::new(),
                metadata: serde_json::json!({}),
                duration_ms: start.elapsed().unwrap_or_default().as_secs_f64() * 1000.0,
                tokens_used: 0, success: false,
                error: Some("Policy denied: query".to_string()),
            };
        }

        let kg_context = context.get("kg_context").and_then(|v| v.as_str()).unwrap_or("none");
        let _use_reasoning = context.get("use_reasoning").and_then(|v| v.as_bool()).unwrap_or(true);

        let research_prompt = format!(
            "{}\n\nResearch the following topic thoroughly:\nTask: {}\nKG Context: {}\n\nProvide findings with key insights, sources, and analysis.",
            self.base.system_prompt, task, kg_context
        );

        let rag_result = context.get("rag_answer").and_then(|v| v.as_str()).unwrap_or("");

        let prompt = if !rag_result.is_empty() {
            format!("{}\n\nRAG Context:\n{}\n\nTask: {}\n\nSynthesize the above context with your knowledge to provide a comprehensive answer.",
                    self.base.system_prompt, rag_result, task)
        } else {
            research_prompt
        };

        let findings = self.base.call_llm(vec![
            serde_json::json!({"role": "system", "content": &self.base.system_prompt}),
            serde_json::json!({"role": "user", "content": &prompt}),
        ], 0.1, 4096).await.unwrap_or_else(|e| format!("Research failed: {}", e));

        let sources_used: Vec<String> = if !rag_result.is_empty() {
            context.get("rag_sources").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                .unwrap_or_default()
        } else {
            vec!["direct_llm".to_string()]
        };

        let duration_ms = start.elapsed().unwrap_or_default().as_secs_f64() * 1000.0;
        self.base.state = AgentState::Complete;
        self.base.task_history.push(serde_json::json!({"task": task, "success": true}));

        AgentResult {
            agent_id: self.base.id.clone(),
            role: "researcher".to_string(),
            task: task.to_string(),
            output: findings.clone(),
            metadata: serde_json::json!({
                "sources": sources_used,
                "kg_context": kg_context,
                "findings_count": 1,
            }),
            duration_ms,
            tokens_used: 0,
            success: true,
            error: None,
        }
    }
}
