# Aetheris — Interface Definitions

## Rust Core Traits

### `AetherisBridge`
Base trait for all bridge implementations.

```rust
pub trait AetherisBridge: Send + Sync {
    fn name(&self) -> &'static str;
    fn health(&self) -> Pin<Box<dyn Future<Output = Result<HealthStatus>> + Send>>;
}
```

### `ModelBridge`
AI model inference (Ollama implementation: `OllamaBridge`).

```rust
pub trait ModelBridge: AetherisBridge {
    fn query(&self, prompt: &str, model: &str, max_tokens: u32)
        -> Pin<Box<dyn Future<Output = Result<String>> + Send>>;
    fn embed(&self, input: &str)
        -> Pin<Box<dyn Future<Output = Result<Vec<f32>>> + Send>>;
    fn embed_and_index(&self, input: &str, collection: &str)
        -> Pin<Box<dyn Future<Output = Result<usize>> + Send>>;
    fn list_models(&self)
        -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send>>;
}
```

### `SecurityBridge`
OPA policy enforcement (implementation: `OpaBridge`).

```rust
pub trait SecurityBridge: AetherisBridge {
    fn check_access(&self, user: &str, action: &str, resource: &str, context: &Value)
        -> Pin<Box<dyn Future<Output = Result<AllowResult>> + Send>>;
}
```

### `Agent`
Agent trait for the orchestrator's multi-agent pipeline.

```rust
pub trait Agent: Send {
    fn role(&self) -> AgentRole;
    fn state(&self) -> AgentState;
    fn execute(&mut self, task: &str, context: &AgentContext)
        -> Pin<Box<dyn Future<Output = Result<AgentResult>> + Send>>;
    fn policy_check(&self, action: &str, resource: &str) -> Result<bool>;
}
```

### Agent Types

```rust
pub enum AgentRole {
    Planner,
    Researcher,
    Coder,
    Reviewer,
}

pub enum AgentState {
    Idle,
    Executing,
    Waiting,
    Complete,
    Failed(String),
}

pub struct AgentResult {
    pub success: bool,
    pub output: String,
    pub agent_id: String,
    pub duration_ms: u64,
    pub metadata: HashMap<String, String>,
}

pub struct AgentContext {
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub max_iterations: u32,
    pub vault_path: PathBuf,
    pub model_bridge: Option<Arc<dyn ModelBridge>>,
}
```

## REST API Schema

### Core Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | System health status |
| GET | `/api/status` | Service status details |
| GET | `/api/agents/status` | Agent pool status |
| GET | `/api/orchestrator/state` | Cross-engine state |
| GET | `/api/orchestrator/forecast` | Resource forecast |
| GET | `/api/mcp/tools` | MCP tool listing |
| GET | `/api/a2a/messages` | A2A message log |
| GET | `/api/tasks` | Recent tasks |
| POST | `/api/query` | RAG query (JSON: query, reasoning_enabled, top_k, reranker_enabled) |
| POST | `/api/ingest/file` | RAG upload + index (multipart) |
| GET | `/api/sources` | Indexed documents |
| DELETE | `/api/sources/{name}` | Remove a source (file + vector chunks) |
| GET / PUT | `/api/config` | Read / merge-update RAG configuration |
| GET | `/api/stats` | RAG statistics |
| GET | `/api/knowledge-graph/stats` | KG statistics |
| GET | `/api/knowledge-graph/entities` | KG entities |
| GET | `/api/knowledge-graph/relations` | KG relations |
| GET | `/api/coordinator/circuits` | Circuit breaker states |
| GET | `/api/v1/models` | AI models list |
| GET | `/api/dev/logs` | System logs |
| GET | `/api/dev/config` | Config files |
| GET | `/api/dev/metrics` | Metrics dashboard |

### Sync Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/sync/status` | Sync system status |
| POST | `/sync/upload` | Upload file to vault |
| GET | `/sync/download/{path}` | Download file from vault |
| DELETE | `/sync/delete/{path}` | Delete file from vault |

### Bridge Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/bridge/ai/query` | Model query via bridge |
| POST | `/bridge/ai/embed` | Embed text via bridge |
| POST | `/bridge/security/check` | OPA policy check via bridge |

### Orchestrator Proxy Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/orchestrator/*` | Forwarded to Python orchestrator |
| GET | `/knowledge-graph/*` | Forwarded to Python KG service |
| GET | `/files/*` | Forwarded to Python file service |

## A2A Protocol

### Message Format
```rust
pub struct A2AMessage {
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: A2AMessageType,
    pub payload: String,
    pub approved: bool,
    pub timestamp: u64,
}

pub enum A2AMessageType {
    TaskRequest,
    TaskResult,
    StatusUpdate,
    Error,
    Coordination,
}
```

### Gateway Interface
```rust
pub struct A2AGateway {
    tx: mpsc::Sender<A2AMessage>,
    rx: Mutex<mpsc::Receiver<A2AMessage>>,
    history: Mutex<Vec<A2AMessage>>,
}
```

## MCP Protocol

### Tool Definition
```rust
pub struct MCPTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub tags: Vec<String>,
}
```

### Server Interface
```rust
pub struct MCPServer {
    tools: Vec<MCPTool>,
}

impl MCPServer {
    pub fn new() -> Self;
    pub fn register_tool(&mut self, tool: MCPTool);
    pub fn list_tools(&self) -> &[MCPTool];
    pub fn execute_tool(&self, name: &str, args: Value)
        -> Pin<Box<dyn Future<Output = Result<String>> + Send>>;
}
```

## WAL (Write-Ahead Log)

### Entry Types
```rust
pub enum WalEntryType {
    FileUpload,
    FileDownload,
    FileDelete,
    AiQuery,
    AgentTask,
    WorkflowRun,
    PolicyCheck,
    ConfigChange,
    SystemEvent,
}

pub struct WalEntry {
    pub sequence: u64,
    pub entry_type: WalEntryType,
    pub timestamp: u64,
    pub data: Value,
}
```

## Proxy Interface

### OrchestratorProxy
```rust
pub struct OrchestratorProxy {
    client: reqwest::Client,
    endpoint: String,
    timeout: Duration,
}

impl OrchestratorProxy {
    pub fn forward(&self, method: Method, path: &str, body: Option<Value>)
        -> Pin<Box<dyn Future<Output = Result<Response>> + Send>>;
    pub fn proxy_request(&self, mut req: Request)
        -> Pin<Box<dyn Future<Output = Result<Response>> + Send>>;
}
```
