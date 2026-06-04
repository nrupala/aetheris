use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
pub struct MCPTool {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}

pub struct MCPServer {
    pub tools: Mutex<HashMap<String, MCPTool>>,
}

impl MCPServer {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let server = Self {
            tools: Mutex::new(HashMap::new()),
        };
        server.register_defaults();
        server
    }

    fn register_defaults(&self) {
        let defaults = vec![
            MCPTool {
                name: "rag_query".to_string(),
                description: "Query the RAG document store".to_string(),
                tags: vec!["rag".to_string(), "search".to_string()],
            },
            MCPTool {
                name: "file_list".to_string(),
                description: "List uploaded files in the vault".to_string(),
                tags: vec!["storage".to_string(), "files".to_string()],
            },
            MCPTool {
                name: "file_upload".to_string(),
                description: "Upload a file to the vault".to_string(),
                tags: vec!["storage".to_string(), "files".to_string()],
            },
            MCPTool {
                name: "ai_query".to_string(),
                description: "Query the local AI model".to_string(),
                tags: vec!["ai".to_string(), "llm".to_string()],
            },
            MCPTool {
                name: "policy_check".to_string(),
                description: "Check OPA policy for authorization".to_string(),
                tags: vec!["security".to_string(), "policy".to_string()],
            },
            MCPTool {
                name: "kg_entities".to_string(),
                description: "Query knowledge graph entities".to_string(),
                tags: vec!["kg".to_string(), "graph".to_string()],
            },
            MCPTool {
                name: "audit_log".to_string(),
                description: "Read audit log entries".to_string(),
                tags: vec!["security".to_string(), "audit".to_string()],
            },
            MCPTool {
                name: "agent_execute".to_string(),
                description: "Execute an agent task".to_string(),
                tags: vec!["agent".to_string(), "orchestration".to_string()],
            },
            MCPTool {
                name: "workflow_run".to_string(),
                description: "Run a multi-agent workflow".to_string(),
                tags: vec!["agent".to_string(), "workflow".to_string()],
            },
            MCPTool {
                name: "openrouter_query".to_string(),
                description: "Query OpenRouter for remote AI model inference (fallback LLM)"
                    .to_string(),
                tags: vec![
                    "ai".to_string(),
                    "openrouter".to_string(),
                    "remote".to_string(),
                ],
            },
            MCPTool {
                name: "exasearch_search".to_string(),
                description: "Search the web via ExaSearch API for context retrieval".to_string(),
                tags: vec![
                    "search".to_string(),
                    "web".to_string(),
                    "exasearch".to_string(),
                ],
            },
            MCPTool {
                name: "fusion_query".to_string(),
                description: "Smart query with local→search→remote fallback chain".to_string(),
                tags: vec![
                    "ai".to_string(),
                    "fusion".to_string(),
                    "fallback".to_string(),
                ],
            },
        ];
        let mut tools = self.tools.lock().unwrap();
        for tool in defaults {
            tools.insert(tool.name.clone(), tool);
        }
    }

    pub fn register_tool(&self, tool: MCPTool) {
        self.tools.lock().unwrap().insert(tool.name.clone(), tool);
    }

    pub fn list_tools(&self) -> Vec<MCPTool> {
        self.tools.lock().unwrap().values().cloned().collect()
    }

    pub fn list_tools_json(&self) -> serde_json::Value {
        let tools = self.list_tools();
        serde_json::json!({ "tools": tools })
    }
}
