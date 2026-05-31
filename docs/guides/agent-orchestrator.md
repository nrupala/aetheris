# Aetheris Agent Orchestrator — Phase 2 & 3 Documentation

## Overview

The Agent Orchestrator is the multi-agent coordination layer of Aetheris v2.0. It implements:
- **Phase 2**: MCP server, multi-agent roles, A2A protocol
- **Phase 3**: Cross-system orchestration, shared KG, resource forecasting

```
┌─────────────────────────────────────────────────────────────────┐
│                    Aetheris v2.0 Architecture                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────┐  ┌─────────┐  ┌─────────────┐  ┌──────────────┐  │
│  │  RAG    │  │   AI    │  │   Agent     │  │     Dev      │  │
│  │ Engine  │  │ Engine  │  │ Orchestrator│  │   Engine     │  │
│  │ :8080   │  │ :1234   │  │   :9090     │  │   :8443      │  │
│  └────┬────┘  └────┬────┘  └──────┬──────┘  └──────┬───────┘  │
│       │            │              │                │           │
│       └────────────┴──────────────┼────────────────┘           │
│                                  │                            │
│                    ┌─────────────▼──────────────┐              │
│                    │    Cross-System Layer       │              │
│                    │                             │              │
│                    │  ┌───────────┐ ┌─────────┐ │              │
│                    │  │ State Sync│ │Resource │ │              │
│                    │  │ Manager   │ │Forecast │ │              │
│                    │  └───────────┘ └─────────┘ │              │
│                    │  ┌───────────┐ ┌─────────┐ │              │
│                    │  │Shared KG  │ │  Spread │ │              │
│                    │  │   Hub     │ │ Forecast│ │              │
│                    │  └───────────┘ └─────────┘ │              │
│                    └─────────────────────────────┘              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Access

- **URL:** `https://agents.nrupalakolkar.com`
- **Auth:** HTTP Basic Auth (same credentials as RAG/AI)
- **Connection:** Cloudflare Tunnel (encrypted, no open ports)

## Phase 2: Agent Orchestrator

### Agent Roles

| Role | Purpose | Model | Key Tools |
|------|---------|-------|-----------|
| Researcher | Query RAG, extract entities to KG | strand-rust-coder-14b-v1 | rag_query, kg_lookup, kg_context |
| Coder | Generate code with RAG context | strand-rust-coder-14b-v1 | file_read, file_write, rag_query |
| Reviewer | Evaluate outputs against standards | microsoft/phi-4-reasoning-plus | kg_lookup, rag_query, evaluate |
| Planner | Decompose tasks, coordinate agents | microsoft/phi-4-reasoning-plus | query_kg, coordinate, list_agents |

### MCP Server

The MCP (Model Context Protocol) server provides standardized tool/resource/prompt interfaces:

**Tools** (8 default):
- `rag_query` — Semantic search against RAG knowledge base
- `rag_ingest` — Index files into the knowledge base
- `kg_lookup` — Look up entities in the knowledge graph
- `kg_context` — Get personal context from KG
- `file_read` — Read files from workspace
- `file_write` — Write files to workspace
- `list_directory` — List workspace directory contents

**Prompts** (5 templates):
- `research_brief` — Structured research brief generation
- `code_review` — Code quality and correctness review
- `task_decomposition` — Break down complex tasks
- `agent_handoff` — Context handoff between agents
- `answer_synthesis` — Synthesize multi-agent findings

### A2A Protocol

Bidirectional communication between agents via file-based message bus:

```
/workspace/intermediate/{conversation_id}/
    ├── msg_abc123.json  (researcher → coder: request)
    ├── msg_def456.json  (coder → reviewer: request)
    └── msg_ghi789.json  (reviewer → planner: response)
```

Each message includes:
- Conversation ID for grouping
- Source and target agent
- Message type (request/response/notification)
- TTL-based cleanup (5 min default)
- OPA policy gate for delivery approval

### OPA Policy Enforcement

Every agent action is gated by OPA policy:

| Agent | Allowed Actions |
|-------|----------------|
| Researcher | query, read, extract_entities, list_sources |
| Coder | write, read, execute_readonly, list_directory |
| Reviewer | read, evaluate, query_kg, list_sources |
| Planner | read, query, query_kg, list_agents, coordinate |

Policy check flow:
1. Agent attempts action
2. Request sent to OPA endpoint
3. OPA evaluates against policy rules
4. If OPA unavailable → local fallback rules
5. If denied → action blocked with error

### Workflow Engine

Multi-agent workflow: `Planner → Researcher → Coder → Reviewer`

1. **Planner** decomposes the task into steps with dependencies
2. **Researcher** queries RAG with reasoning loop, extracts entities to KG
3. **Coder** uses RAG context + KG for code generation
4. **Reviewer** evaluates output against KG history + RAG standards

Each step communicates via A2A protocol with OPA-gated message delivery.

### API Reference

```
POST /task/submit          — Submit single-agent task
GET  /task/{id}            — Get task status
GET  /tasks                — List recent tasks
POST /workflow/run         — Run multi-agent workflow
GET  /agents               — List agents
GET  /agents/status        — Agent status
POST /mcp/request          — MCP protocol endpoint
GET  /mcp/tools            — List MCP tools
GET  /mcp/prompts          — List MCP prompts
GET  /health               — Health check
```

## Phase 3: Cross-System Orchestration

### 3.1: Cross-Engine State Manager

Synchronized state across all 4 engines (RAG, AI, Dev, Agents):

- **Atomic state updates** via file-based WAL
- **Change subscriptions** for reactive components
- **Crash recovery** via snapshot/restore
- **Dashboard** with per-engine status

State tracked per engine:
- Status (healthy, degraded, down, prewarming)
- Response time, memory, active tasks, queue depth
- Error count (1h rolling window)

### 3.2: Resource Forecaster

Predicts resource needs using sliding window analysis:

- **Linear regression** on recent memory/CPU metrics
- **Trend detection** for growth patterns
- **Confidence scoring** based on data variance
- **Pre-warm recommendations** when predicted need > threshold

### 3.3: Shared Knowledge Graph Hub

Single source of truth KG shared across all engines:

- **Per-engine permissions** (read/write/query)
- **Change log** for audit trail
- **Access log** for monitoring
- **Conflict resolution** for concurrent writes
- **Export/Import** for backup and migration

Engine permissions:
| Engine | Read | Write Entities | Write Relations | Query |
|--------|------|---------------|----------------|-------|
| RAG | ✓ | ✓ | ✓ | ✓ |
| AI | ✓ | — | — | ✓ |
| Dev | ✓ | — | ✓ | ✓ |
| Agents | ✓ | ✓ | ✓ | ✓ |

### 3.4: Spread Forecaster

Comprehensive resource allocation forecasting:

- **Per-engine projections** with confidence scores
- **Host capacity analysis** (memory + CPU)
- **Bottleneck detection** (highest utilization ratio)
- **Actionable recommendations** generated automatically

Recommendation types:
- Memory pressure warnings
- CPU oversubscription alerts
- Pre-warm scheduling (stagger to avoid spikes)
- Bottleneck engine identification

### API Reference (Phase 3)

```
GET  /orchestrator/state          — Cross-engine state dashboard
POST /orchestrator/state/engine/* — Update engine state
GET  /orchestrator/forecast       — Resource spread forecast
GET  /orchestrator/kg-hub         — Shared KG dashboard
POST /orchestrator/kg-hub/write   — Write to shared KG
GET  /orchestrator/snapshot       — Create state snapshot
GET  /a2a/messages                — A2A message log
```

## Deployment

### Docker Compose

```yaml
llmvm-orchestrator:
  build: ../LLMVM (Dockerfile.orchestrator)
  ports: 9090:9090
  volumes:
    - orchestrator_data:/workspace
    - rag_core:/workspace/rag_core
    - agent_orchestrator:/workspace/agent_orchestrator
  environment:
    - LMSTUDIO_ENDPOINT=http://host.docker.internal:1234
    - OPA_ENDPOINT=http://opa-gateway:8181
    - WORKSPACE_ROOT=/workspace
```

### Nginx Configuration

`agents.nrupalakolkar.com` routes to the orchestrator:
- Static UI served from `/usr/share/nginx/html/agents/`
- API endpoints proxied to `llmvm_orchestrator:9090`
- HTTP Basic Auth for all access

## Project Structure

```
agent_orchestrator/
├── __init__.py           # Package metadata v2.0.0
├── config.py             # Orchestrator configuration
├── server.py             # FastAPI server (all endpoints)
├── a2a_gateway.py        # A2A protocol + OPA gate
├── cross_system.py       # Phase 3: state, forecast, KG hub
├── mcp/
│   ├── __init__.py       # MCP module exports
│   ├── server.py         # MCP server (tools, resources, prompts)
│   ├── tools.py          # Default tool implementations
│   └── prompts.py        # Default prompt templates
├── agents/
│   ├── __init__.py       # Agent module exports
│   └── base.py           # All agent roles with RAG/KG/OPA
```

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| MCP + A2A dual protocol | MCP for tool access, A2A for peer coordination (arXiv 2601.13671) |
| File-based message bus | Zero dependencies, durable, easy to debug, TTL cleanup |
| OPA with local fallback | Zero-trust by default, degraded operation when OPA down |
| SQLite-backed state | Reuses coordinator pattern, atomic writes, crash recovery |
| Linear regression forecast | Simple, effective for short-term predictions, low compute |
| Per-engine KG permissions | Least privilege — each engine only accesses what it needs |
| Shared KG as single source | Eliminates data silos, enables cross-engine context |
