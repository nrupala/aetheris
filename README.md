# Aetheris — Sovereign AI-Native Personal Cloud

Zero-trust personal cloud with AI-powered policy enforcement, secure storage, multi-agent orchestration, and local LLM inference. Built with Rust (Axum), OPA, Ollama, and Docker.

## Architecture

```
Browser → Cloudflare Tunnel → Nginx (auth + proxy) → Rust Core (:8080)
  ├── Ollama (:11434) — Local LLM inference (qwen2.5:7b default)
  └── Python Orchestrator (:9090) — RAG pipeline, KG (optional, --profile llmvm)
```

## Quick Start

### Prerequisites
- Rust 1.75+
- Docker + Docker Compose v2
- A Linux server (production) or local machine (development)

### Local Development
```bash
cd core
cargo build --release
./target/release/aetheris
```

### Docker Stack
```bash
docker compose build
docker compose up -d

# With LLMVM (agent orchestrator + RAG)
docker compose --profile llmvm up -d
```

## Subdomains

| Domain | Purpose | Auth |
|--------|---------|------|
| dev.nrupalakolkar.com | Dev Sandbox — API Console, logs, config, metrics | `dev_user` |
| ai.nrupalakolkar.com | AI Chat — local LLM inference | `ai_user` |
| rag.nrupalakolkar.com | Document Q&A — RAG pipeline | `rag_user` |
| agents.nrupalakolkar.com | Agent Orchestrator — multi-agent workflows | `dev_user` |

All auth users share the password: `BCjfTYIIjMASFGVM`

## Services

| Service | Container | Port | Description |
|---------|-----------|------|-------------|
| Core | aetheris_core | 8080 | Rust Axum API server |
| Nginx | aetheris_nginx | 443 | Reverse proxy + HTTP Basic Auth |
| Ollama | aetheris_ollama | 11434 | Local LLM inference (qwen2.5:7b default) |
| Orchestrator | aetheris_orchestrator | 9090 | RAG pipeline, KG (LLMVM profile) |

## Configuration

Model names are resolved from environment variables at startup. Fallbacks default to models that stay installed on the host, so retiring an older model never requires a rebuild.

| Env Var | Default | Purpose |
|---------|---------|---------|
| `AI_ENDPOINT` | `http://localhost:11434` | Ollama base URL |
| `AI_MODEL` | `qwen2.5:7b` | Default generation model (also the fallback for `AetherisConnector`) |
| `AETHERIS_FALLBACK_MODEL` | `qwen2.5:7b` | Generation fallback when no model is specified |
| `AETHERIS_EMBED_FALLBACK_MODEL` | `nomic-embed-text` | Embedding model fallback |
| `OPA_ENDPOINT` | `http://opa:8181` | OPA policy engine |
| `PORT` | `8080` | HTTP listen port |
| `VAULT_PATH` | `vault` | Data/vector-store directory |

## Project Structure

```
aetheris/
├── core/                    # Rust source code
│   ├── src/
│   │   ├── main.rs         # Axum server — 38+ endpoints
│   │   ├── bridge.rs       # ModelBridge, SecurityBridge, AetherisBridge
│   │   ├── implementation.rs  # OllamaBridge + OpaBridge
│   │   ├── agents/         # 4 agents: Planner, Researcher, Coder, Reviewer
│   │   ├── a2a.rs          # Agent-to-Agent messaging
│   │   ├── mcp.rs          # MCP protocol tools
│   │   ├── wal.rs          # Write-ahead log (9 entry types)
│   │   ├── sync.rs         # File sync handlers
│   │   └── proxy.rs        # Reverse proxy to Python orchestrator
│   └── Cargo.toml
├── web/                    # Subdomain HTML UIs
├── docs/                   # Documentation (AppDocs format)
├── scripts/                # bootstrap.sh, verification.sh, killswitch.sh
└── compose.yaml            # Docker Compose (+ LLMVM profile)
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `GET /api/health` | GET | System health with agent/task count |
| `GET /api/status` | GET | Component status details |
| `GET /api/agents/status` | GET | Agent pool status |
| `GET /api/orchestrator/state` | GET | Cross-engine state |
| `GET /api/orchestrator/forecast` | GET | Resource spread forecast |
| `GET /api/mcp/tools` | GET | MCP tool listing |
| `POST /api/workflow/run` | POST | Run multi-agent workflow |
| `POST /api/query` | POST | RAG document query |
| `POST /api/ingest/file` | POST | Upload document |
| `GET /api/sources` | GET | List indexed documents |
| `GET /api/dev/logs` | GET | WAL-backed system logs |
| `GET /api/dev/config` | GET | Runtime configuration |
| `GET /api/dev/metrics` | GET | Service metrics |

## Security

- **Zero-trust**: Every request evaluated by OPA policies (default deny)
- **Local AI**: All LLM inference runs locally on Ollama — no cloud API calls
- **Encrypted tunnel**: Cloudflare Tunnel provides TLS from browser to proxy
- **Basic auth**: Subdomain access gated by Nginx HTTP Basic Auth
- **WAL audit**: All file and system operations logged to append-only WAL

## Documentation

See `docs/README.md` for the full documentation index. Key documents:

- [Getting Started](docs/guides/getting-started.md)
- [API Reference](docs/api-reference/README.md)
- [Common Commands](docs/COMMANDS.md)
- [FAQ](docs/FAQ.md)
- [What Not To Do](docs/WHAT_NOT_TO_DO.md)
- [Role-Based Guides](docs/ROLE_GUIDES.md)

## Emergency

```bash
# Health check
bash scripts/verification.sh

# Emergency shutdown (irreversible!)
sudo bash scripts/killswitch.sh
```
