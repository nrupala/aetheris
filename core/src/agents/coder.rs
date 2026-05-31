use async_trait::async_trait;
use std::sync::Arc;
use std::time::SystemTime;

use super::{Agent, AgentResult, AgentRole, AgentState, AgentStatus, BaseAgent};
use crate::bridge::{ModelBridge, SecurityBridge};

pub struct CoderAgent {
    base: BaseAgent,
}

impl CoderAgent {
    pub fn new(
        model: String,
        system_prompt: String,
        model_bridge: Option<Arc<dyn ModelBridge>>,
        security_bridge: Option<Arc<dyn SecurityBridge>>,
    ) -> Self {
        Self {
            base: BaseAgent::new(
                AgentRole::Coder,
                model,
                if system_prompt.is_empty() {
                    "You are a code generation agent. Write clean, well-documented, production-ready code.".to_string()
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
impl Agent for CoderAgent {
    fn id(&self) -> &str {
        &self.base.id
    }
    fn role(&self) -> AgentRole {
        AgentRole::Coder
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

        if !self.base.check_policy("read", task).await {
            self.base.state = AgentState::Failed;
            return AgentResult {
                agent_id: self.base.id.clone(),
                role: "coder".to_string(),
                task: task.to_string(),
                output: String::new(),
                metadata: serde_json::json!({}),
                duration_ms: start.elapsed().unwrap_or_default().as_secs_f64() * 1000.0,
                tokens_used: 0,
                success: false,
                error: Some("Policy denied: read".to_string()),
            };
        }

        let code_context = context
            .get("rag_answer")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let kg_context = context
            .get("kg_context")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let workspace = context
            .get("workspace")
            .and_then(|v| v.as_str())
            .unwrap_or("/workspace");

        let prompt = format!(
            "{}\n\n## Task\n{}\n\n## Context from Knowledge Base\n{}\n\n## Personal Context (Knowledge Graph)\n{}\n\n## Workspace\n{}\n\nProvide a complete, working implementation with error handling and usage examples.",
            self.base.system_prompt, task, code_context, kg_context, workspace
        );

        let code_output = self
            .base
            .call_llm(
                vec![
                    serde_json::json!({"role": "system", "content": &self.base.system_prompt}),
                    serde_json::json!({"role": "user", "content": &prompt}),
                ],
                0.1,
                4096,
            )
            .await
            .unwrap_or_else(|e| format!("// Code generation failed: {}", e));

        let duration_ms = start.elapsed().unwrap_or_default().as_secs_f64() * 1000.0;
        self.base.state = AgentState::Complete;
        self.base
            .task_history
            .push(serde_json::json!({"task": task, "success": true}));

        AgentResult {
            agent_id: self.base.id.clone(),
            role: "coder".to_string(),
            task: task.to_string(),
            output: code_output.clone(),
            metadata: serde_json::json!({
                "workspace": workspace,
                "kg_context_used": !kg_context.is_empty(),
                "rag_context_used": !code_context.is_empty(),
            }),
            duration_ms,
            tokens_used: 0,
            success: true,
            error: None,
        }
    }
}
