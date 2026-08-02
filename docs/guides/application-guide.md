# Aetheris — Application & Usage Guide

> **Your Sovereign AI-Native Personal Cloud**
> Zero-trust, zero-knowledge, self-hosted. Replaces commercial cloud services with an encrypted, AI-powered FOSS mesh.

---

## Table of Contents

1. [What is Aetheris?](#1-what-is-aetheris)
2. [Who Is This For?](#2-who-is-this-for)
3. [Use Cases & Application Scenarios](#3-use-cases--application-scenarios)
   - [3.1 Personal Knowledge Base with AI Search](#31-personal-knowledge-base-with-ai-search)
   - [3.2 Private AI Coding Assistant](#32-private-ai-coding-assistant)
   - [3.3 Secure File Sync & Vault](#33-secure-file-sync--vault)
   - [3.4 Home Server / Lab Orchestration](#34-home-server--lab-orchestration)
   - [3.5 Small Team Collaboration](#35-small-team-collaboration)
   - [3.6 Agentic Workflow Automation](#36-agentic-workflow-automation)
   - [3.7 Security Honeypot & Threat Intelligence](#37-security-honeypot--threat-intelligence)
   - [3.8 Air-Gapped / Offline AI](#38-air-gapped--offline-ai)
   - [3.9 OCI / Edge Deployment](#39-oci--edge-deployment)
   - [3.10 Emergency Data Protection](#310-emergency-data-protection)
4. [Architecture Overview](#4-architecture-overview)
5. [Deployment Options](#5-deployment-options)
6. [Usage Workflows](#6-usage-workflows)
7. [LLMVM Deep-Dive](#7-llmvm-deep-dive)
8. [Integration Patterns](#8-integration-patterns)
9. [Extending Aetheris](#9-extending-aetheris)
   - [9.1 Extension Architecture](#91-extension-architecture)
   - [9.2 Adding New Models](#92-extending-with-new-models)
   - [9.3 Adding New Agents](#93-extending-with-new-agents)
   - [9.4 Adding MCP Tools, Resources, Prompts](#94-extending-with-new-mcp-tools)
   - [9.5 A2A Protocol Extensions](#95-extending-with-a2a-protocol)
   - [9.6 Extension Discovery](#96-extension-discovery--monitoring)
   - [9.7 Best Practices](#97-extension-best-practices)
10. [Limitations & Considerations](#10-limitations--considerations)
11. [Roadmap & Coming Soon](#11-roadmap--coming-soon)

---

## 1. What is Aetheris?

Aetheris is a **sovereign, AI-native personal cloud** that runs on your own hardware. It combines:

- **A Rust core** — secure file operations, OPA-based authorization, Prometheus metrics, Write-Ahead Log
- **A Python LLMVM** — Retrieval-Augmented Generation (RAG) pipeline, multi-agent orchestrator, knowledge graph, Model Context Protocol (MCP)
- **Zero-Trust mesh networking** — WireGuard-encrypted tunnels, no public ports
- **Encrypted storage** — ZFS AES-256-GCM at rest, snapshot-based recovery

Every capability runs **locally**. No data leaves your infrastructure unless you explicitly replicate it.

---

## 2. Who Is This For?

| Persona | Why Aetheris |
|---------|-------------|
| **Solo developer / engineer** | Private AI coding assistant, codebase Q&A, personal knowledge base |
| **Privacy-conscious individual** | Replace Google Drive, Dropbox, Notion AI with self-hosted equivalent |
| **Security researcher** | High-interaction honeypot, OPA policy experimentation, network forensics |
| **Homelab enthusiast** | Self-hosted AI stack, WireGuard mesh, ZFS storage management |
| **Small team (2-5 people)** | Shared knowledge base, agentic workflows, encrypted collaboration |
| **Edge/IoT operator** | Offline-capable AI, air-gapped operation, low-touch deployment |

---

## 3. Use Cases & Application Scenarios

### 3.1 Personal Knowledge Base with AI Search

**The Problem:** You have notes, code snippets, articles, research papers scattered across files. Finding information is slow. Commercial tools leak your data to third parties.

**How Aetheris Solves It:**

Upload any text, code, or config file into the RAG pipeline. The LLMVM chunks, embeds (via local LMStudio/Ollama), and indexes everything into a SQLite + NumPy vector store. Then ask natural-language questions — answers come with source citations.

**Workflow:**

```
User uploads documents → RAG Chunker splits text → Embedder generates vectors
→ VectorStore indexes → User queries → Retriever finds relevant chunks
→ Generator synthesizes answer with citations
```

**Example commands:**

```bash
# Upload documentation
curl -X POST "http://localhost:8081/ingest/file" -F "file=@notes.md"

# Ask a question
curl -X POST "http://localhost:8081/query" \
  -H "Content-Type: application/json" \
  -d '{"query": "How do I configure WireGuard on this system?"}'

# With advanced options
curl -X POST "http://localhost:8081/query" \
  -H "Content-Type: application/json" \
  -d '{"query": "What are the ZFS snapshot policies?", "top_k": 5, "threshold": 0.8}'

# Check what's indexed
curl "http://localhost:8081/stats"
```

**Best for:**
- Developer documentation (API docs, README files, architecture notes)
- Research papers and article archives
- Project-specific knowledge (configs, deployment notes, troubleshooting guides)
- Meeting notes and decision logs

---

### 3.2 Private AI Coding Assistant

**The Problem:** GitHub Copilot, Cursor, and other AI coding tools send code to external servers. You cannot use them with proprietary or classified codebases.

**How Aetheris Solves It:**

Run the Agent Orchestrator with local LLM backends (LMStudio, Ollama, GPT4All). The `CoderAgent` and `ReviewerAgent` operate entirely on your hardware. The MCP subsystem provides tools (`file_read`, `file_write`, `rag_query`, `list_directory`) that agents use to understand and modify your codebase.

**Workflow:**

```
User submits task → PlannerAgent decomposes → ResearcherAgent queries RAG/KG
→ CoderAgent generates code → ReviewerAgent checks quality/security
→ Results returned with full audit trail
```

**Example commands:**

```bash
# Submit a coding task
curl -X POST "http://localhost:9090/agent/task" \
  -H "Content-Type: application/json" \
  -d '{"task": "Add error handling to the upload endpoint", "agent": "coder"}'

# Run full multi-agent workflow
curl -X POST "http://localhost:9090/agent/task" \
  -H "Content-Type: application/json" \
  -d '{"task": "Refactor the auth module to use async/await", "workflow": true}'

# Use MCP tools directly
curl -X POST "http://localhost:9090/tools/rag_query" \
  -H "Content-Type: application/json" \
  -d '{"query": "How is authentication handled?"}'
```

**Best for:**
- Proprietary codebase analysis and refactoring
- Security-sensitive development environments
- Offline/air-gapped development
- Code review automation for private repos

---

### 3.3 Secure File Sync & Vault

**The Problem:** Dropbox, Google Drive, and iCloud scan your files. Breaches expose everything. You want encrypted sync without trusting a third party.

**How Aetheris Solves It:**

The Rust core provides file upload/download with OPA authorization at every step. Files are stored on ZFS with AES-256-GCM encryption. The Write-Ahead Log records every operation. ZFS snapshots at 15-minute intervals enable point-in-time recovery. WireGuard mesh connects all your devices without exposing public HTTP ports.

**Workflow:**

```
Device connects via WireGuard → Authenticates → OPA evaluates authorization
→ File upload/download via Rust core → ZFS encrypted storage
→ WAL logs operation → ZREPL replicates snapshots offsite
```

**Example commands:**

```bash
# Upload a file (behind WireGuard)
curl -X POST "http://10.0.0.1:8080/upload" \
  -F "file=@confidential.docx" \
  -H "Authorization: Bearer $TOKEN"

# Download with integrity check
curl -O "http://10.0.0.1:8080/download/confidential.docx" \
  -H "Authorization: Bearer $TOKEN"

# List snapshots for recovery
zfs list -t snapshot -r aetheris_vault

# Rollback to previous state
sudo ./scripts/vault_rollback.sh aetheris_vault@zrepl_2026-05-27_1500
```

**Best for:**
- Replacing Dropbox/Google Drive for sensitive files
- Backup target for important documents
- Sync across personal devices (laptop, phone, server)
- Legal/financial document storage

---

### 3.4 Home Server / Lab Orchestration

**The Problem:** Running a home server means juggling Docker containers, VPNs, storage, backups, and monitoring — all with different tools and configs.

**How Aetheris Solves It:**

Aetheris bundles everything into a single Docker Compose stack with one `bootstrap.sh` install. It includes:

- **VictoriaMetrics** for time-series monitoring
- **AI Sentinel** for automated threat detection
- **ZREPL** for ZFS snapshot replication
- **Nginx** reverse proxy with SSL
- **Dynamic port allocation** to avoid conflicts

**Workflow:**

```bash
# Full install
sudo ./scripts/bootstrap.sh

# Check status
curl "http://10.0.0.1:8080/status"
curl "http://10.0.0.1:8428/health"  # VictoriaMetrics

# Monitor with AI Sentinel (runs every 60s)
docker logs aetheris_sentinel -f

# View metrics
curl "http://10.0.0.1:8080/metrics"
```

**Best for:**
- Homelab infrastructure management
- Media server with encrypted storage
- Personal VPN hub with AI monitoring
- Development/staging environments

---

### 3.5 Small Team Collaboration

**The Problem:** Small teams need shared knowledge, secure file exchange, and coordination tools — but Slack/Teams/Notion leak data to SaaS providers and cost too much.

**How Aetheris Solves It:**

Multiple users connect via WireGuard. The Knowledge Graph builds shared context across team members. Multi-agent workflows coordinate tasks (planner → researcher → coder → reviewer). A2A (Agent-to-Agent) protocol enables inter-agent communication with OPA policy gates.

**Workflow:**

```
Team members join WireGuard mesh → Shared KG accumulates knowledge
→ Agents route tasks based on context → Results stored in A2A message bus
→ Audit trail for every operation
```

**Example:**

```bash
# Team member checks KG context
curl "http://10.0.0.1:9090/knowledge-graph/context" \
  -d '{"scope": "team-knowledge-base"}'

# Submit a shared task
curl -X POST "http://10.0.0.1:9090/workflow/run" \
  -H "Content-Type: application/json" \
  -d '{"task": "Document the deployment process", "agents": ["planner", "researcher"]}'

# Check A2A message history
curl "http://10.0.0.1:9090/a2a/messages?scope=team-knowledge-base"
```

**Best for:**
- Startup teams that want data sovereignty
- Open-source project maintainers
- Research groups sharing findings
- Legal/medical teams with compliance requirements

---

### 3.6 Agentic Workflow Automation

**The Problem:** Automating multi-step tasks (research → plan → code → review → deploy) requires stitching together multiple tools and APIs. Most solutions are fragile and don't learn from experience.

**How Aetheris Solves It:**

The Processing Coordinator orchestrates complex workflows with a state machine, circuit breaker, and self-evaluator. The coordinator learns from past runs (performance anomalies, failure patterns) and adjusts behavior. The Spread Forecaster predicts resource bottlenecks and recommends pre-warming.

**Architecture:**

```
Task → QueueController → ProcessingCoordinator (state machine)
  → Agent selection (auto-routed) → Execution with circuit breaker
  → SelfEvaluator (anomaly detection) → Results with audit trail
  → ResourceForecaster (predictive scaling)
```

**Example:**

```bash
# Submit workflow
curl -X POST "http://localhost:9090/orchestrator/workflow" \
  -H "Content-Type: application/json" \
  -d '{
    "task": "Research best practices for Rust async error handling, then update our codebase",
    "workflow": "research_then_code"
  }'

# Check forecaster (predictive analytics)
curl "http://localhost:9090/orchestrator/forecast"

# View coordinator state
curl "http://localhost:9090/orchestrator/state"
```

**Best for:**
- Automated code review pipelines
- Research → summarize → publish workflows
- Infrastructure-as-code generation and validation
- Continuous documentation updates

---

### 3.7 Security Honeypot & Threat Intelligence

**The Problem:** You don't know who's probing your infrastructure or what they're after. Traditional security tools are expensive and complex.

**How Aetheris Solves It:**

The Ghost Shell is a high-interaction honeypot that runs in an isolated container with minimal privileges. It logs attacker behavior. The AI Sentinel reads audit logs every 60 seconds, analyzes patterns via LLM, and flags SAFE/WARNING/CRITICAL threats. OPA auto-bans peers after 5 failures.

**Workflow:**

```
Attacker probes → Ghost Shell captures interaction → Audit logs written
→ AI Sentinel analyzes every 60s → Threat level determined
→ OPA auto-bans on threshold breach → Grafana dashboard visualizes
```

**Example:**

```bash
# Check AI Sentinel status
docker logs aetheris_sentinel --tail 20

# View Grafana dashboard (if configured)
open http://aetheris.local:3000/d/sentinel

# Review banned peers
curl "http://10.0.0.1:8080/status"
# Response includes "security_violations" metric

# Emergency kill-switch (if compromised)
sudo ./scripts/killswitch.sh --dry-run  # preview first
sudo ./scripts/killswitch.sh             # execute
```

**Best for:**
- Security researchers analyzing attack patterns
- Production servers needing threat intelligence
- Compliance environments requiring audit trails
- Red team / blue team practice environments

---

### 3.8 Air-Gapped / Offline AI

**The Problem:** Many AI applications require internet connectivity. Classified, military, or remote operations need AI that works entirely offline.

**How Aetheris Solves It:**

The LLMVM is designed for local-only operation. All AI backends (LMStudio, Ollama, GPT4All) run on the same machine. The Model Router abstracts multiple backends with automatic fallback. No telemetry, no external API calls, no data leakage.

**Workflow:**

```
Install models offline (sneaker-net) → Start Aetheris with no network
→ All RAG + agent operations use local LLM → Results stay on device
→ Checkpoint system enables safe experimentation
```

**Requirements:**
- Pre-downloaded models (Mistral, Llama, Nomic-embed-text, etc.)
- 16GB+ RAM for 7B parameter models
- CPU-only mode available (slower, no GPU required)

**Best for:**
- Classified / government environments
- Military field operations
- Remote research stations
- Privacy-critical healthcare or legal analysis

---

### 3.9 OCI / Edge Deployment

**The Problem:** Deploying AI infrastructure to cloud VMs or edge devices requires handling ARM architecture, limited resources, and unreliable networks.

**How Aetheris Solves It:**

The codebase includes ARM-compatible Dockerfiles and OCI deployment scripts. The Resource Monitor (part of the Processing Coordinator) adapts to available memory/CPU. The circuit breaker prevents cascading failures when resources are constrained.

**Example (OCI ARM deployment):**

```bash
# Deployment utilities included
python3 create_arm_instance.py  # Provision ARM VM
./deploy.sh                      # Deploy Aetheris stack

# Resource-aware coordinator
curl "http://localhost:9090/orchestrator/forecast"
# Returns: {"cpu_bottleneck": false, "memory_pressure": "low", "recommendations": [...]}
```

**Best for:**
- ARM-based cloud VMs (Ampere, Graviton)
- Edge gateways with limited resources
- Raspberry Pi clusters (basic file/security services)
- Remote field deployments with intermittent connectivity

---

### 3.10 Emergency Data Protection

**The Problem:** If your server is compromised or you're under legal pressure to hand over data, you need a way to destroy encryption keys and make data irrecoverable.

**How Aetheris Solves It:**

The Kill-Switch protocol performs a forensic-grade purge:

1. Stops all Docker containers
2. Shuts down WireGuard interfaces
3. Unmounts the ZFS encrypted vault
4. Unloads encryption keys from kernel keyring
5. Shreds the master key file (3-pass overwrite + zero)
6. Clears bash history, syslogs, and temporary files

**Usage:**

```bash
# Always dry-run first
sudo ./scripts/killswitch.sh --dry-run

# Execute when needed
sudo ./scripts/killswitch.sh
```

**Best for:**
- Emergency breach response
- Legal confiscation scenarios
- Decommissioning hardware
- End-of-life data sanitization

---

## 4. Architecture Overview

```
                          WIREGUARD MESH (10.0.0.0/24)
                         Only UDP 51820 exposed publicly
                                   │
            ┌──────────────────────┼──────────────────────┐
            ▼                      ▼                      ▼
    ┌───────────────┐     ┌───────────────┐     ┌───────────────┐
    │  Rust Core    │     │  LLMVM RAG    │     │  Agent Orch.  │
    │  :8080        │     │  :8081        │     │  :9090        │
    │               │     │               │     │               │
    │ • File ops    │◄───►│ • Chunker     │◄───►│ • Researcher  │
    │ • OPA auth    │     │ • Embedder    │     │ • Coder       │
    │ • Metrics     │     │ • VectorStore │     │ • Reviewer    │
    │ • WAL         │     │ • Generator   │     │ • Planner     │
    │ • Watcher     │     │ • KG          │     │ • MCP server  │
    └───────┬───────┘     └───────┬───────┘     └───────┬───────┘
            │                     │                     │
            ▼                     ▼                     ▼
    ┌───────────────┐     ┌───────────────┐     ┌───────────────┐
    │  ZFS Vault    │     │  AI Backend   │     │  Victoria     │
    │  AES-256-GCM  │     │  LMStudio     │     │  Metrics      │
    │  + Snapshots  │     │  / Ollama     │     │  :8428        │
    └───────────────┘     └───────────────┘     └───────────────┘
                                   │
                                   ▼
                          ┌───────────────┐
                          │  OPA Policy   │
                          │  Engine       │
                          │  :8181        │
                          └───────────────┘
                                   │
                                   ▼
                          ┌───────────────┐
                          │  AI Sentinel  │
                          │  Threat       │
                          │  Detection    │
                          └───────────────┘
```

### Key Data Flows

| Flow | Path | Protocol |
|------|------|----------|
| File Upload | Client → WireGuard → Rust Core → ZFS Vault | HTTP/WireGuard |
| AI Query | Client → RAG Service → VectorStore → LMStudio → Client | HTTP |
| Agent Task | Client → Orchestrator → Agent(s) → MCP tools → RAG/KG | HTTP |
| Security | OPA → Rust Core → WAL → AI Sentinel (every 60s) | HTTP/internal |
| Monitoring | All services → VictoriaMetrics → Grafana | HTTP |
| Backup | ZFS → ZREPL → Offsite/USB | SSH/ZFS send |

---

## 5. Deployment Options

### Option 1: Single Server (Recommended for most users)

```bash
git clone https://github.com/nrupala/aetheris.git
cd aetheris
sudo ./scripts/bootstrap.sh
```

- One machine runs everything
- WireGuard connects client devices
- ZFS for local storage + snapshots

### Option 2: Docker Compose (Development / Testing)

```bash
# Without LLMVM (lighter)
docker compose up -d

# With LLMVM (requires AI models)
docker compose --profile llmvm up -d
```

### Option 3: CI / Automated Testing

Runs in GitHub Actions via UAT workflow. Verifies all services, security policies, file operations, and LLMVM integration.

### Option 4: ARM / Edge

```bash
# Use ARM-compatible Dockerfiles
docker compose -f compose.yaml -f ci-compose.yaml build
```

---

## 6. Usage Workflows

### 6.1 First-Time Setup

```bash
# 1. Clone and bootstrap
git clone https://github.com/nrupala/aetheris.git
cd aetheris
sudo ./scripts/bootstrap.sh

# 2. Verify installation
./scripts/verification.sh

# 3. Configure WireGuard peers (edit /etc/wireguard/aetheris.conf)
# 4. Connect from client devices
# 5. Access dashboard at http://10.0.0.1:8080
```

### 6.2 Daily Operations

```bash
# Check system health
curl http://localhost:8080/health

# View metrics
curl http://localhost:8080/metrics

# Upload a document to RAG
curl -X POST http://localhost:8081/ingest/file -F "file=@document.md"

# Query the knowledge base
curl -X POST http://localhost:8081/query \
  -H "Content-Type: application/json" \
  -d '{"query": "your question here"}'

# Run an agent task
curl -X POST http://localhost:9090/agent/task \
  -H "Content-Type: application/json" \
  -d '{"task": "Summarize recent changes"}'

# Check coordinator status
curl http://localhost:9090/orchestrator/state
```

### 6.3 Maintenance

```bash
# Create ZFS snapshot
sudo zfs snapshot aetheris_vault/secure_data@manual-$(date +%Y%m%d)

# List snapshots
sudo zfs list -t snapshot -r aetheris_vault

# Update containers
docker compose pull
docker compose up -d

# Port allocation check
bash scripts/port_allocator.sh --show
```

### 6.4 Recovery

```bash
# Rollback ZFS snapshot
sudo ./scripts/vault_rollback.sh aetheris_vault@zrepl_2026-05-27_1500

# USB cold storage backup
sudo ./scripts/usb_cold_storage.sh /dev/sdb1

# Emergency kill-switch
sudo ./scripts/killswitch.sh --dry-run
```

---

## 7. LLMVM Deep-Dive

The LLMVM (LLM Virtual Machine) is the AI brain of Aetheris. It is a self-contained, locally-run AI pipeline with three major subsystems:

### 7.1 RAG Pipeline (`rag_core/`)

| Component | File | Function |
|-----------|------|----------|
| TextChunker | `chunker.py` | Splits documents into semantic chunks using tiktoken, paragraph-aware |
| Embedder | `embedder.py` | Batches embeddings via LMStudio `/v1/embeddings`, auto-detects dimensions |
| VectorStore | `vector_store.py` | SQLite + NumPy cosine similarity search, no external vector DB |
| Retriever | `retriever.py` | Multi-query expansion + Reciprocal Rank Fusion (RRF) for better recall |
| Generator | `generator.py` | Context-aware answer synthesis with source attribution + streaming |
| KnowledgeGraph | `knowledge_graph.py` | Entity extraction, relation mapping, user profile, interaction history |
| ReasoningLoop | `reasoning_loop.py` | Iterative search → think → answer → verify with temperature annealing |
| ModelRouter | `model_router.py` | Multi-provider abstraction (LMStudio, Ollama, OpenAI, Anthropic, Custom) |
| Pipeline | `pipeline.py` | End-to-end: ingest, query (with/without reasoning), list, delete, reset |

### 7.2 Agent Orchestrator (`agent_orchestrator/`)

| Component | File | Function |
|-----------|------|----------|
| 4 Agents | `agents/base.py` | Researcher, Coder, Reviewer, Planner — each with OPA gate + KG/RAG |
| Server | `server.py` | FastAPI with 18+ endpoints for tasks, workflows, MCP, state, A2A, health |
| MCP Server | `mcp/server.py` | Model Context Protocol: tool registry, resource server, prompt library |
| MCP Tools | `mcp/tools.py` | 8 tools: rag_query, rag_ingest, kg_lookup, kg_context, file_read, file_write, list_directory, code_execute |
| A2A Protocol | `a2a_gateway.py` | Agent-to-Agent message bus with OPA policy gate, 4 message types, TTL |
| Cross-System | `cross_system.py` | WAL-backed state engine, resource forecaster, shared KG hub, spread analysis |
| Coordinator | `coordinator.py` | State machine, circuit breaker, resource monitor, error classifier, audit logger, self-evaluator |

### 7.3 AI Backends Supported

| Backend | Type | Config |
|---------|------|--------|
| LMStudio | Local (OpenAI-compatible) | `http://host.docker.internal:1234` |
| Ollama | Local (loopback) | `http://127.0.0.1:11434` |
| GPT4All | Local | Native Python library |
| OpenAI | Remote (optional) | API key required |
| Anthropic | Remote (optional) | API key required |
| Custom | Any OpenAI-compatible | Configurable endpoint |

---

## 8. Integration Patterns

### 8.1 curl / HTTP API

Every capability is accessible via REST. This is the primary integration method.

```bash
# Health check
curl http://localhost:8080/health

# RAG query
curl -X POST http://localhost:8081/query -d '{"query":"..."}'

# Agent task
curl -X POST http://localhost:9090/agent/task -d '{"task":"..."}'
```

### 8.2 MCP Integration

The Model Context Protocol server allows any MCP-compatible client (Claude Desktop, Cursor, etc.) to use Aetheris tools directly.

```bash
# List available tools
curl http://localhost:9090/mcp/tools

# Call a tool
curl -X POST http://localhost:9090/tools/rag_query \
  -H "Content-Type: application/json" \
  -d '{"query": "What is the ZFS snapshot policy?"}'
```

### 8.3 Docker Compose Stack

Add Aetheris services to your existing Docker Compose setup by extending `compose.yaml`. All services use environment variables for configuration.

### 8.4 Prometheus / Grafana

VictoriaMetrics collects metrics from all services. Import `monitoring/sentinel_dashboard.json` into Grafana for pre-built visualizations.

### 8.5 CI/CD Pipeline

GitHub Actions workflows demonstrate automated UAT. The `Build and Test` workflow compiles Rust, builds Docker images, and runs the full UAT suite.

---

## 9. Extending Aetheris

Aetheris is designed with first-class extension support across four dimensions: **Models**, **Agents**, **MCP Tools**, and **MCP Prompts**. All extensions plug in without modifying core code — they register at runtime via API handshakes or configuration.

---

### 9.1 Extension Architecture

```
┌─────────────────────────────────────────────────────────┐
│                 EXTENSION LAYER                          │
│                                                         │
│  ┌────────────┐  ┌────────────┐  ┌───────────────────┐  │
│  │ New Models  │  │ New Agents │  │ New MCP Tools/     │  │
│  │ (Provider) │  │ (Role)     │  │ Prompts/Resources  │  │
│  └─────┬──────┘  └─────┬──────┘  └────────┬──────────┘  │
│        │               │                  │             │
│        ▼               ▼                  ▼             │
│  ┌────────────────────────────────────────────────┐    │
│  │          REGISTRY / DISCOVERY LAYER             │    │
│  │  ModelRouter  │  Agent Factory  │  MCP Server   │    │
│  └────────────────────────────────────────────────┘    │
│        │               │                  │             │
│        ▼               ▼                  ▼             │
│  ┌────────────────────────────────────────────────┐    │
│  │                CORE EXECUTION                    │    │
│  │  RAG Pipeline  │  Orchestrator  │  Coordinator   │    │
│  └────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

Every extension registers itself so it can be discovered, invoked, and monitored through existing APIs.

---

### 9.2 Extending with New Models

Aetheris supports any OpenAI-compatible or Anthropic API endpoint. You add models by registering them with the `ModelRouter`.

#### How Model Routing Works

The `ModelRouter` (`rag_core/model_router.py:85`) maintains an ordered chain of `ModelInfo` entries. On each request, it tries the primary model first, then falls back through the chain:

```
Primary (priority=0, GPU) → Secondary (priority=1, CPU) → Fallback → Error
```

#### Adding a Model via Config (No Code)

Set environment variables before starting the orchestrator:

```bash
# Add Llama 3.2 via Ollama
export DEFAULT_MODEL="llama3.2"
export LMSTUDIO_ENDPOINT="http://127.0.0.1:11434"

# Add fallback to GPT4All CPU model
export FALLBACK_MODEL="mistral-7b-instruct"
export GPT4ALL_ENDPOINT="http://gpt4all:4891"
```

#### Adding a Model Programmatically

```python
from rag_core.model_router import ModelRouter, ModelInfo, Provider, ModelCapability

# Register a new model provider
router = ModelRouter([
    ModelInfo(
        name="gpt-4o",
        provider=Provider.OPENAI,
        endpoint="https://api.openai.com/v1",
        capabilities=[ModelCapability.CHAT, ModelCapability.VISION,
                      ModelCapability.STRUCTURED_OUTPUT, ModelCapability.TOOL_USE],
        api_key=os.environ["OPENAI_API_KEY"],
        priority=0,
    ),
    ModelInfo(
        name="claude-sonnet-4",
        provider=Provider.ANTHROPIC,
        endpoint="https://api.anthropic.com/v1",
        capabilities=[ModelCapability.CHAT, ModelCapability.TOOL_USE],
        api_key=os.environ["ANTHROPIC_API_KEY"],
        priority=1,
    ),
])
```

#### Supported Providers

| Provider | Enum Value | Endpoint Pattern | Capabilities |
|----------|-----------|-----------------|--------------|
| LMStudio | `Provider.LMSTUDIO` | `http://host:1234/v1` | chat, embedding, structured_output |
| Ollama | `Provider.OLLAMA` | `http://127.0.0.1:11434/v1` | chat, embedding |
| GPT4All | `Provider.GPT4ALL` | `http://host:4891/v1` | chat |
| OpenAI | `Provider.OPENAI` | `https://api.openai.com/v1` | chat, vision, structured_output, tool_use |
| Anthropic | `Provider.ANTHROPIC` | `https://api.anthropic.com/v1` | chat, tool_use |
| Custom | `Provider.CUSTOM` | Any OpenAI-compatible | configurable |

#### Adding a New Provider Type

To add a completely new provider (e.g., Google Gemini, AWS Bedrock):

1. Add the provider to the `Provider` enum in `model_router.py:31`
2. Add a `_call_<provider>()` method to `ModelRouter`
3. Add the routing dispatch in `_call_provider()` at line 150

```python
# Step 1: Add enum value
class Provider(str, Enum):
    GEMINI = "gemini"       # new

# Step 2: Add dispatch in _call_provider
def _call_provider(self, model, ...):
    if model.provider == Provider.GEMINI:
        return self._call_gemini(model, ...)

# Step 3: Implement the call method
def _call_gemini(self, model, messages, ...):
    # Implement Gemini API call
    # Return ModelResponse
```

#### API Endpoints for Model Discovery & Routing

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `GET /orchestrator/state` | GET | View currently loaded models per engine |
| `GET /health` | GET | Returns agent count, tool count, model info |
| `POST /agent/task` | POST | Auto-routes to correct model based on task role |

The orchestrator's `/health` endpoint returns model routing information:

```json
{
  "status": "healthy",
  "agents": 4,
  "tools": 8,
  "prompts": 5,
  "spread_forecast": {
    "total_memory_mb": 15360,
    "confidence": 0.85
  }
}
```

---

### 9.3 Extending with New Agents

Aetheris comes with 4 built-in agents (Researcher, Coder, Reviewer, Planner). You add new agents by extending the `BaseAgent` class and registering them in the orchestrator config.

#### How Agent Registration Works

At startup, the orchestrator reads `config.agent_roles`, `config.role_models`, and `config.role_prompts` from `OrchestratorConfig` (`agent_orchestrator/config.py:42`). For each role, it calls `create_agent()` and stores the agent in a global dict. Agents are then discoverable via `GET /agents`.

#### Adding an Agent via Config (No Code)

```python
# In agent_orchestrator/config.py, add to the existing config:
agent_roles = [
    "researcher", "coder", "reviewer", "planner",
    "security_auditor",    # new agent role
]

role_models = {
    "researcher": "strand-rust-coder-14b-v1",
    "coder": "strand-rust-coder-14b-v1",
    "reviewer": "microsoft/phi-4-reasoning-plus",
    "planner": "microsoft/phi-4-reasoning-plus",
    "security_auditor": "microsoft/phi-4-reasoning-plus",  # model for new agent
}

role_prompts = {
    "security_auditor": (
        "You are a Security Audit Agent. Your role is to analyze code and "
        "configurations for security vulnerabilities, check OPA policy compliance, "
        "identify OWASP Top 10 issues, and recommend mitigations. "
        "Be thorough and provide CVE references where applicable."
    ),
}
```

The orchestrator auto-creates the agent at startup — no code changes needed.

#### Adding an Agent Programmatically

For custom logic (tool access, specialized KG queries), subclass `BaseAgent`:

```python
from agent_orchestrator.agents.base import BaseAgent, AgentResult

class SecurityAuditorAgent(BaseAgent):
    """Custom agent for security auditing."""

    def __init__(self, opa_gate, **kwargs):
        super().__init__(role="security_auditor", opa_gate=opa_gate, **kwargs)

    async def execute(self, task: str, context: dict) -> AgentResult:
        # 1. Check OPA policy
        allowed = await self.opa_gate.check(self.role, "audit", {"task": task})
        if not allowed:
            return AgentResult(agent_id=self.id, role=self.role,
                               task=task, output="", success=False,
                               error="OPA policy denied")

        # 2. Query RAG for known vulnerabilities
        rag_results = await context["rag_client"].query(
            f"Security patterns for: {task}"
        )

        # 3. Run LLM analysis with structured output
        result = await self.llm.chat_structured(
            messages=[{"role": "system", "content": self.prompt},
                      {"role": "user", "content": task}],
            response_format=SecurityAuditReport,
        )

        # 4. Log to Knowledge Graph
        if context.get("kg_client"):
            await context["kg_client"].record_interaction(
                engine=self.role,
                interaction_type="audit",
                details={"task": task, "findings": result.dict()},
            )

        return AgentResult(
            agent_id=self.id, role=self.role, task=task,
            output=result.json(), success=True,
            metadata={"vulnerabilities_found": len(result.issues)},
        )
```

Then register it with the orchestrator:

```python
# In server.py startup, after existing agent creation:
from agent_orchestrator.agents.base import SecurityAuditorAgent

security_agent = SecurityAuditorAgent(opa_gate)
agents[security_agent.id] = security_agent
logger.info(f"Custom agent registered: {security_agent.id}")
```

#### API Handshake Endpoints for Agents

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `GET /agents` | GET | Lists all registered agents with role, status, model |
| `GET /agents/status` | GET | Agent health: total, idle, busy counts per agent |
| `POST /task/submit` | POST | Submit task to specific agent role (auto-created if missing) |
| `GET /task/{id}` | GET | Check task status and results |
| `POST /workflow/run` | POST | Run multi-agent workflow including custom agents |

Registration handshake flow:

```
1. Orchestrator starts → reads config.agent_roles
2. For each role → calls create_agent(role, model, prompt, opa_gate)
3. Agent stored in agents dict with unique ID
4. Agent available via GET /agents immediately
5. POST /task/submit with role="security_auditor" routes to the new agent
6. If agent not found → auto-creates on first request (server.py:299-302)
```

#### Agent Auto-Creation on First Use

If you submit a task to a role that doesn't exist yet, the orchestrator auto-creates it:

```bash
# Submit task to "security_auditor" — auto-created if missing
curl -X POST "http://localhost:9090/task/submit" \
  -H "Content-Type: application/json" \
  -d '{"task": "Audit the upload endpoint for OWASP vulnerabilities", "role": "security_auditor"}'
```

This uses the model and prompt from `config.role_models` and `config.role_prompts`.

---

### 9.4 Extending with New MCP Tools

The MCP (Model Context Protocol) subsystem lets you register custom tools that agents can discover and invoke at runtime.

#### How MCP Tool Registration Works

The `ToolRegistry` (`mcp/server.py:51`) uses a decorator pattern. Registering a tool makes it available via `tools/list` and `tools/call` MCP methods. Built-in defaults are in `mcp/tools.py`.

#### Adding a Tool via Decorator

```python
from agent_orchestrator.mcp.server import MCPServer

mcp = MCPServer()

@mcp.tools.register(
    name="vulnerability_scan",
    description="Scan a codebase for known security vulnerabilities",
    parameters={
        "type": "object",
        "properties": {
            "repo_path": {
                "type": "string",
                "description": "Path to the repository to scan"
            },
            "severity": {
                "type": "string",
                "enum": ["low", "medium", "high", "critical"],
                "description": "Minimum severity threshold",
                "default": "medium"
            },
        },
        "required": ["repo_path"],
    },
    tags=["security", "scan", "audit"],
)
async def vulnerability_scan(repo_path: str, severity: str = "medium") -> dict:
    """Custom tool: scan codebase for vulnerabilities."""
    results = run_safety_scan(repo_path)
    filtered = [v for v in results if v["severity"] >= severity]
    return {
        "vulnerabilities_found": len(filtered),
        "details": filtered,
        "summary": f"Scanned {repo_path}, found {len(filtered)} {severity}+ issues",
    }
```

#### Adding a Tool to an Existing MCP Server

```python
from agent_orchestrator.mcp.server import MCPTool

custom_tool = MCPTool(
    name="deploy_service",
    description="Deploy a service to the Aetheris infrastructure",
    parameters={
        "type": "object",
        "properties": {
            "service_name": {"type": "string"},
            "image": {"type": "string"},
            "port": {"type": "integer"},
        },
        "required": ["service_name", "image"],
    },
    handler=deploy_service_handler,
    tags=["deploy", "infrastructure"],
)

mcp_server.tools.register_tool(custom_tool)
```

#### Adding MCP Resources

Resources expose data as URI-addressable content (files, configs, state):

```python
from agent_orchestrator.mcp.server import MCPResource

async def get_system_config():
    with open("/etc/aetheris/config.json") as f:
        return f.read()

mcp_server.resources.add_resource(MCPResource(
    uri="aetheris://config/system",
    name="System Configuration",
    description="Current system configuration in JSON",
    mime_type="application/json",
    content_fn=get_system_config,
))
```

#### Adding MCP Prompts

Prompts are reusable templates for agent workflows:

```python
from agent_orchestrator.mcp.server import MCPPrompt

mcp_server.prompts.add_prompt(MCPPrompt(
    name="security_review",
    description="Generate a structured security review report",
    arguments=[
        {"name": "target", "description": "What to review", "required": "true"},
        {"name": "compliance_standard", "description": "e.g., OWASP, SOC2, ISO27001",
         "required": "false"},
    ],
    template="""You are a Security Review Agent.

Review Target: {target}
Compliance Standard: {compliance_standard}

Generate a security review following:
1. **Threat Model** — Assets, attackers, attack surface
2. **Vulnerability Scan** — Findings by severity
3. **Compliance Gap** — Missing controls
4. **Remediation Plan** — Prioritized fixes
5. **Risk Score** — Overall risk rating (1-10)""",
))
```

#### API Handshake Endpoints for MCP

| Endpoint | Method | MCP Method | Purpose |
|----------|--------|-----------|---------|
| `POST /mcp/request` | POST | `initialize` | MCP handshake — returns server info + capabilities |
| `POST /mcp/request` | POST | `tools/list` | List all registered tools with schemas |
| `POST /mcp/request` | POST | `tools/call` | Execute a tool by name |
| `POST /mcp/request` | POST | `resources/list` | List all registered resources |
| `POST /mcp/request` | POST | `resources/read` | Read a resource by URI |
| `POST /mcp/request` | POST | `prompts/list` | List all registered prompt templates |
| `POST /mcp/request` | POST | `prompts/render` | Render a prompt with arguments |
| `GET /mcp/tools` | GET | — | List tools (direct HTTP shortcut) |
| `GET /mcp/prompts` | GET | — | List prompts (direct HTTP shortcut) |
| `GET /mcp/resources` | GET | — | List resources (direct HTTP shortcut) |

#### MCP Handshake Flow

```
Client → POST /mcp/request {"method": "initialize"}
  ← {"name": "aetheris-orchestrator", "version": "2.0.0",
     "capabilities": {"tools": {"list": true, "call": true}, ...}}

Client → POST /mcp/request {"method": "tools/list"}
  ← {"tools": [{"name": "rag_query", "description": "...", "inputSchema": {...}}, ...]}

Client → POST /mcp/request {"method": "tools/call",
         "params": {"name": "vulnerability_scan", "arguments": {"repo_path": "/src"}}}
  ← {"success": true, "result": {"vulnerabilities_found": 3, ...}}
```

#### Example: Register a Tool and Call It via MCP

```bash
# Step 1: Initialize MCP connection
curl -X POST "http://localhost:9090/mcp/request" \
  -H "Content-Type: application/json" \
  -d '{"method": "initialize"}'

# Step 2: List available tools
curl -X POST "http://localhost:9090/mcp/request" \
  -H "Content-Type: application/json" \
  -d '{"method": "tools/list"}'

# Step 3: Call a tool
curl -X POST "http://localhost:9090/mcp/request" \
  -H "Content-Type: application/json" \
  -d '{"method": "tools/call",
       "params": {"name": "rag_query",
                   "arguments": {"query": "How is authentication handled?",
                                 "top_k": 3}}}'

# Or use direct HTTP shortcut
curl "http://localhost:9090/mcp/tools"
```

---

### 9.5 Extending with A2A Protocol

The Agent-to-Agent protocol (`a2a_protocol.py`) lets you add new message types, engine types, and communication patterns.

#### Adding a New Message Type

```python
from rag_core.a2a_protocol import MessageType, A2AMessage, MessageFactory

# Add new message type
class MessageType(str, Enum):
    QUERY = "query"
    RESPONSE = "response"
    ERROR = "error"
    STATUS = "status"
    CONTEXT = "context"
    COMMAND = "command"
    FEEDBACK = "feedback"        # new message type

# Create a factory method
@staticmethod
def coordinator_feedback(conversation_id: str, feedback: str,
                         score: float) -> A2AMessage:
    return A2AMessage(
        message_id=str(uuid.uuid4()),
        conversation_id=conversation_id,
        from_engine="coordinator",
        to_engine="rag",
        message_type=MessageType.FEEDBACK.value,
        payload={"feedback": feedback, "score": score},
        priority=MessagePriority.NORMAL.value,
    )
```

#### Adding a New Engine Type

```python
class EngineType(str, Enum):
    RAG = "rag"
    AI = "ai"
    DEV = "dev"
    COORDINATOR = "coordinator"
    SECURITY = "security"    # new engine
```

Then add allowed communication patterns in the OPA policy gate:

```python
allowed_patterns = {
    ("rag", "ai"), ("ai", "rag"),
    ("ai", "dev"), ("dev", "ai"),
    ("security", "coordinator"),   # new pattern
    ("coordinator", "security"),   # new pattern
}
```

---

### 9.6 Extension Discovery & Monitoring

Every extension is automatically discoverable through existing endpoints:

| What | Discovery Endpoint | Registration Method |
|------|-------------------|-------------------|
| **Models** | `GET /health` (shows model info per agent) | Config env vars or `ModelInfo` object |
| **Agents** | `GET /agents`, `GET /agents/status` | Config `agent_roles` or `create_agent()` |
| **MCP Tools** | `POST /mcp/request` (method: `tools/list`), `GET /mcp/tools` | `@registry.register()` decorator |
| **MCP Resources** | `POST /mcp/request` (method: `resources/list`), `GET /mcp/resources` | `resource_server.add_resource()` |
| **MCP Prompts** | `POST /mcp/request` (method: `prompts/list`), `GET /mcp/prompts` | `prompt_library.add_prompt()` |
| **A2A Engines** | `GET /a2a/messages` | `A2AMessageBus` + `OPAPolicyGate` |
| **Orchestrator State** | `GET /orchestrator/state` | `CrossEngineState.update_engine()` |

---

### 9.7 Extension Best Practices

1. **Model priority** — Set primary=0 for GPU models, higher numbers for CPU fallbacks. The ModelRouter auto-falls back if primary fails.

2. **Agent system prompts** — Be specific about the agent's role, tools it can use, and output format. Well-defined prompts produce reliable agent behavior.

3. **MCP tool design** — Fewer, broader tools beat many narrow ones. Each tool should solve a complete task. Use `tags` for tool categorization.

4. **A2A messages** — Set `ttl_seconds` to prevent stale messages. Use `requires_ack=True` for critical inter-agent commands.

5. **OPA policy** — Every agent action and A2A message goes through OPA. Add Rego rules for new agent roles to maintain zero-trust.

6. **Testing extensions** — Submit a task to a new agent via `POST /task/submit`, check `GET /task/{id}` for results, verify with `GET /agents/status`.

7. **Resource cleanup** — Register a cleanup handler for MCP resources that hold file handles or connections.

---

## 10. Limitations & Considerations

| Area | Limitation | Mitigation |
|------|-----------|------------|
| **PDF support** | Not directly supported | Convert to text first |
| **Image/video analysis** | Not supported | Text-based analysis only |
| **Scalability** | Single-server design | Horizontal scaling not planned |
| **LLM quality** | Depends on local model | Use 7B+ parameter models for best results |
| **GPU requirement** | CPU-only is slow | Use LMStudio with GPU passthrough |
| **Network** | WireGuard required | All access is VPN-gated |
| **ZFS requirement** | Linux-only filesystem | Use ext4 + encryption as fallback |
| **Storage** | 500GB minimum for production | SSD recommended for vector search |

---

## 11. Roadmap & Coming Soon

| Feature | Status | Phase |
|---------|--------|-------|
| Rust core + OPA auth | ✅ Complete | Phase 1 |
| ZFS vault + snapshots | ✅ Complete | Phase 1 |
| WireGuard mesh | ✅ Complete | Phase 1 |
| RAG pipeline | ✅ Complete | Phase 2 |
| Multi-agent orchestrator | ✅ Complete | Phase 2 |
| Knowledge Graph | ✅ Complete | Phase 2 |
| MCP protocol | ✅ Complete | Phase 2 |
| LLMVM integration | ✅ Complete | Phase 2 |
| Cross-system orchestration | 🔄 In Progress | Phase 3 |
| Spread forecasting | 🔄 In Progress | Phase 3 |
| Shared KG hub | 🔄 In Progress | Phase 3 |
| Mobile client SDK | 📋 Planned | Phase 3 |
| WebSocket events | 📋 Planned | Phase 3 |
| Plugin system | 📋 Planned | Phase 4 |

---

## Appendix A: Quick Reference Card

```bash
# CORE
http://localhost:8080/status      # System status
http://localhost:8080/health       # Health check
http://localhost:8080/metrics      # Prometheus metrics
POST /upload                       # File upload
GET /download/{filename}           # File download

# RAG
http://localhost:8081/health       # RAG health
POST /query                        # Ask a question
POST /ingest/file                  # Upload document
GET /stats                         # Index statistics
GET /sources                       # List indexed sources
DELETE /sources/{path}             # Remove a source

# ORCHESTRATOR
http://localhost:9090/health       # Orchestrator health
POST /agent/task                   # Submit task (auto-creates agent if new role)
GET /agents                        # List registered agents
GET /agents/status                 # Agent health (idle/busy counts)
POST /workflow/run                 # Multi-agent workflow
POST /mcp/request                  # MCP protocol: initialize, tools/*, resources/*, prompts/*
GET /mcp/tools                     # List MCP tools (shortcut)
GET /mcp/prompts                   # List MCP prompts (shortcut)
GET /mcp/resources                 # List MCP resources (shortcut)
POST /orchestrator/state/engine/{name}  # Update engine state
GET /orchestrator/state            # Cross-engine state dashboard
GET /orchestrator/forecast         # Resource spread forecast
GET /orchestrator/kg-hub           # Shared KG dashboard
POST /orchestrator/kg-hub/write    # Write to shared KG
GET /orchestrator/snapshot         # Create state snapshot
GET /a2a/messages                  # A2A message log

# INFRASTRUCTURE
http://localhost:8428/health       # VictoriaMetrics
http://localhost:8181/health       # OPA health
```

---

*Generated: 2026-05-27*
*Version: 2.0.0*
