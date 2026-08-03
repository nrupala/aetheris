pub mod coder;
pub mod planner;
pub mod researcher;
pub mod reviewer;

use async_trait::async_trait;
use serde::Serialize;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bridge::{ModelBridge, SecurityBridge};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum AgentRole {
    Researcher,
    Coder,
    Reviewer,
    Planner,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::Researcher => "researcher",
            AgentRole::Coder => "coder",
            AgentRole::Reviewer => "reviewer",
            AgentRole::Planner => "planner",
        }
    }
}

impl FromStr for AgentRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "researcher" => Ok(AgentRole::Researcher),
            "coder" => Ok(AgentRole::Coder),
            "reviewer" => Ok(AgentRole::Reviewer),
            "planner" => Ok(AgentRole::Planner),
            _ => Err(format!("unknown AgentRole: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum AgentState {
    Idle,
    Thinking,
    Executing,
    Waiting,
    Complete,
    Failed,
}

impl AgentState {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentState::Idle => "idle",
            AgentState::Thinking => "thinking",
            AgentState::Executing => "executing",
            AgentState::Waiting => "waiting",
            AgentState::Complete => "complete",
            AgentState::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentResult {
    pub agent_id: String,
    pub role: String,
    pub task: String,
    pub output: String,
    pub metadata: serde_json::Value,
    pub duration_ms: f64,
    pub tokens_used: u64,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentStatus {
    pub id: String,
    pub role: String,
    pub model: String,
    pub state: String,
    pub tasks_completed: u64,
    pub policy_checks: u64,
    pub policy_allowed: u64,
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> &str;
    fn role(&self) -> AgentRole;
    fn model(&self) -> &str;
    fn state(&self) -> AgentState;
    fn tasks_completed(&self) -> u64;
    fn policy_checks(&self) -> u64;
    fn policy_allowed(&self) -> u64;

    async fn execute(&mut self, task: &str, context: &serde_json::Value) -> AgentResult;
    fn get_status(&self) -> AgentStatus;
    fn reset(&mut self);
}

pub struct BaseAgent {
    pub id: String,
    pub role: AgentRole,
    pub model: String,
    pub system_prompt: String,
    pub state: AgentState,
    pub model_bridge: Option<Arc<dyn ModelBridge>>,
    pub security_bridge: Option<Arc<dyn SecurityBridge>>,
    pub task_history: Vec<serde_json::Value>,
    pub policies_checked: u64,
    pub policies_allowed: u64,
}

impl BaseAgent {
    pub fn new(
        role: AgentRole,
        model: String,
        system_prompt: String,
        model_bridge: Option<Arc<dyn ModelBridge>>,
        security_bridge: Option<Arc<dyn SecurityBridge>>,
    ) -> Self {
        let id = format!(
            "{}_{:x}",
            role.as_str(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        Self {
            id,
            role,
            model,
            system_prompt,
            state: AgentState::Idle,
            model_bridge,
            security_bridge,
            task_history: Vec::new(),
            policies_checked: 0,
            policies_allowed: 0,
        }
    }

    pub async fn check_policy(&mut self, action: &str, _task: &str) -> bool {
        self.policies_checked += 1;
        let allowed = if let Some(ref bridge) = self.security_bridge {
            let authz_input = crate::bridge::AuthzInput {
                identity: String::new(),
                role: self.role.as_str().to_string(),
                method: String::new(),
                path: String::new(),
                action: action.to_string(),
            };
            let opa_allowed = bridge.authorize_agent(&authz_input).await;
            let enforce = bridge.enforcing();
            if !opa_allowed {
                log::warn!(
                    "OPA would DENY agent {} action {} (enforce={})",
                    self.role.as_str(),
                    action,
                    enforce
                );
                crate::metrics::SECURITY_VIOLATIONS.inc();
                // Shadow (enforce off): log the would-deny but allow so agents keep
                // working while we observe. Enforce: honor the deny.
                !enforce
            } else {
                true
            }
        } else {
            let local_allowed: bool = match self.role {
                AgentRole::Researcher => matches!(
                    action,
                    "query" | "read" | "extract_entities" | "list_sources"
                ),
                AgentRole::Coder => matches!(
                    action,
                    "write" | "read" | "execute_readonly" | "list_directory"
                ),
                AgentRole::Reviewer => {
                    matches!(action, "read" | "evaluate" | "query_kg" | "list_sources")
                }
                AgentRole::Planner => matches!(
                    action,
                    "read" | "query" | "query_kg" | "list_agents" | "coordinate"
                ),
            };
            local_allowed
        };
        if allowed {
            self.policies_allowed += 1;
        }
        allowed
    }

    pub async fn call_llm(
        &self,
        messages: Vec<serde_json::Value>,
        _temperature: f64,
        _max_tokens: u64,
    ) -> Result<String, String> {
        if let Some(ref bridge) = self.model_bridge {
            let system = messages
                .iter()
                .find(|m| m["role"] == "system")
                .and_then(|m| m["content"].as_str())
                .unwrap_or("You are a helpful assistant.");
            let user = messages
                .iter()
                .find(|m| m["role"] == "user")
                .and_then(|m| m["content"].as_str())
                .unwrap_or("");
            let prompt = format!("{}\n\n{}", system, user);
            bridge.query(&prompt, &self.model).await
        } else {
            Err("No model bridge configured".to_string())
        }
    }
}

pub fn create_agent(
    role: AgentRole,
    model: String,
    system_prompt: String,
    model_bridge: Option<Arc<dyn ModelBridge>>,
    security_bridge: Option<Arc<dyn SecurityBridge>>,
) -> Box<dyn Agent> {
    match role {
        AgentRole::Researcher => Box::new(researcher::ResearcherAgent::new(
            model,
            system_prompt,
            model_bridge,
            security_bridge,
        )),
        AgentRole::Coder => Box::new(coder::CoderAgent::new(
            model,
            system_prompt,
            model_bridge,
            security_bridge,
        )),
        AgentRole::Reviewer => Box::new(reviewer::ReviewerAgent::new(
            model,
            system_prompt,
            model_bridge,
            security_bridge,
        )),
        AgentRole::Planner => Box::new(planner::PlannerAgent::new(
            model,
            system_prompt,
            model_bridge,
            security_bridge,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{AetherisBridge, AuthzInput};

    struct FakeSecurity {
        allow_agent: bool,
        enforce: bool,
    }

    #[async_trait]
    impl AetherisBridge for FakeSecurity {
        fn name(&self) -> &str {
            "fake"
        }
        async fn health_check(&self) -> bool {
            true
        }
    }
    #[async_trait]
    impl SecurityBridge for FakeSecurity {
        async fn authorize(&self, _input: &AuthzInput) -> bool {
            true
        }
        async fn authorize_agent(&self, _input: &AuthzInput) -> bool {
            self.allow_agent
        }
        fn enforcing(&self) -> bool {
            self.enforce
        }
    }

    fn base_with(bridge: Arc<dyn SecurityBridge>) -> BaseAgent {
        BaseAgent::new(
            AgentRole::Researcher,
            "model".to_string(),
            String::new(),
            None,
            Some(bridge),
        )
    }

    #[tokio::test]
    async fn check_policy_advisory_allows_on_would_deny() {
        // Shadow (enforce off): OPA denies but the agent advisory-allows.
        let mut a = base_with(Arc::new(FakeSecurity {
            allow_agent: false,
            enforce: false,
        }));
        assert!(a.check_policy("query", "task").await);
    }

    #[tokio::test]
    async fn check_policy_enforce_honors_deny() {
        // Enforce on: OPA denies -> hard block.
        let mut a = base_with(Arc::new(FakeSecurity {
            allow_agent: false,
            enforce: true,
        }));
        assert!(!a.check_policy("query", "task").await);
    }

    #[tokio::test]
    async fn check_policy_allows_when_opa_allows() {
        // OPA allows -> allowed in both modes.
        let mut a = base_with(Arc::new(FakeSecurity {
            allow_agent: true,
            enforce: true,
        }));
        assert!(a.check_policy("query", "task").await);
    }

    #[tokio::test]
    async fn check_policy_enforce_blocks_opa_allow_never() {
        // allow_agent=true + enforce=true -> always allowed (no false negative).
        let mut a = base_with(Arc::new(FakeSecurity {
            allow_agent: true,
            enforce: false,
        }));
        assert!(a.check_policy("query", "task").await);
    }

    #[tokio::test]
    async fn check_policy_no_bridge_uses_local_allowlist() {
        let mut a = BaseAgent::new(
            AgentRole::Researcher,
            "model".to_string(),
            String::new(),
            None,
            None,
        );
        assert!(a.check_policy("query", "task").await);
        assert!(!a.check_policy("write", "task").await);
    }
}
