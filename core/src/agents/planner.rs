use async_trait::async_trait;
use std::sync::Arc;
use std::time::SystemTime;

use super::{Agent, AgentResult, AgentRole, AgentState, AgentStatus, BaseAgent};
use crate::bridge::{ModelBridge, SecurityBridge};

pub struct PlannerAgent {
    base: BaseAgent,
}

impl PlannerAgent {
    pub fn new(
        model: String,
        system_prompt: String,
        model_bridge: Option<Arc<dyn ModelBridge>>,
        security_bridge: Option<Arc<dyn SecurityBridge>>,
    ) -> Self {
        Self {
            base: BaseAgent::new(
                AgentRole::Planner,
                model,
                if system_prompt.is_empty() {
                    "You are a planning agent. Break down complex tasks into actionable steps with agent assignments.".to_string()
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
impl Agent for PlannerAgent {
    fn id(&self) -> &str {
        &self.base.id
    }
    fn role(&self) -> AgentRole {
        AgentRole::Planner
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

        if !self.base.check_policy("coordinate", task).await {
            self.base.state = AgentState::Failed;
            return AgentResult {
                agent_id: self.base.id.clone(),
                role: "planner".to_string(),
                task: task.to_string(),
                output: String::new(),
                metadata: serde_json::json!({}),
                duration_ms: start.elapsed().unwrap_or_default().as_secs_f64() * 1000.0,
                tokens_used: 0,
                success: false,
                error: Some("Policy denied: coordinate".to_string()),
            };
        }

        let available_agents = context
            .get("available_agents")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let plan_prompt = format!(
            "Decompose this task into steps and assign each to the most appropriate agent.\n\nTask: {}\nAvailable agents: {:?}\nKnowledge Graph context: {}\n\nReturn a JSON plan with: original_task, steps (array with id, description, agent, depends_on, estimated_ms), total_steps.",
            task, available_agents,
            context.get("kg_context").and_then(|v| v.as_str()).unwrap_or("none")
        );

        let plan_output = self
            .base
            .call_llm(
                vec![
                    serde_json::json!({"role": "system", "content": &self.base.system_prompt}),
                    serde_json::json!({"role": "user", "content": &plan_prompt}),
                ],
                0.1,
                4096,
            )
            .await
            .unwrap_or_else(|e| {
                format!(
                    "{{\"original_task\": \"{}\", \"steps\": [], \"error\": \"{}\"}}",
                    task, e
                )
            });

        let plan: serde_json::Value = serde_json::from_str(&plan_output)
            .unwrap_or_else(|_| serde_json::json!({
                "original_task": task,
                "steps": [{"id": 1, "description": format!("Process: {}", task), "agent": available_agents.first().map(|s| s.as_str()).unwrap_or("researcher"), "depends_on": [], "estimated_ms": 30000}],
            }));

        let duration_ms = start.elapsed().unwrap_or_default().as_secs_f64() * 1000.0;
        self.base.state = AgentState::Complete;
        self.base
            .task_history
            .push(serde_json::json!({"task": task, "success": true}));

        AgentResult {
            agent_id: self.base.id.clone(),
            role: "planner".to_string(),
            task: task.to_string(),
            output: serde_json::to_string_pretty(&plan).unwrap_or_default(),
            metadata: plan,
            duration_ms,
            tokens_used: 0,
            success: true,
            error: None,
        }
    }
}
