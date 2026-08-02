# Aetheris — Sovereign AI-Native Personal Cloud

Zero-trust personal cloud with AI-powered policy enforcement, secure storage, multi-agent orchestration, and local LLM inference. Built with Rust (Axum), OPA, and Ollama; deployed natively (systemd) behind Cloudflare Access — no Docker.

## Architecture

```
Browser → Cloudflare Access (edge auth) → Cloudflare Tunnel → cloudflared → aetheris-core (127.0.0.1:8080)
  ├── Ollama (127.0.0.1:11434) — Local LLM inference (qwen2.5:7b default)
  └── Python Orchestrator (127.0.0.1:9090) — RAG pipeline, KG (optional, --profile llmvm)
```

## Quick Start

### Prerequisites
- Rust 1.75+ (with the matching musl target; the installer adds it)
- A Linux server (production) or local machine (development)
- Ollama on `127.0.0.1:11434` with `qwen2.5:7b` + `nomic-embed-text` pulled
- `cloudflared` (for the public tunnel) — optional for local dev

### Local Development
```bash
cd core
cargo build --release
./target/release/aetheris-core
```

### Native Deploy (no Docker)
The core runs as a native `systemd` service. cloudflared points straight at the
loopback core and Cloudflare Access gates the public hostname.
```bash
sudo scripts/install-native.sh
systemctl status aetheris-core
```
See **[docs/DEPLOY_NATIVE.md](docs/DEPLOY_NATIVE.md)** for the full runbook (build,
install, cloudflared ingress, Cloudflare Access, verification, teardown, rollback).

## Hostnames

| Domain | Purpose | Auth |
|--------|---------|------|
| core.nrupalakolkar.com | Aetheris Core — API + web UIs | Cloudflare Access |
| dev.nrupalakolkar.com | Dev Sandbox — API Console, logs, config, metrics | Cloudflare Access |
| ai.nrupalakolkar.com | AI Chat — local LLM inference | Cloudflare Access |
| rag.nrupalakolkar.com | Document Q&A — RAG pipeline | Cloudflare Access |
| agents.nrupalakolkar.com | Agent Orchestrator — multi-agent workflows | Cloudflare Access |

Access is identity-gated by Cloudflare Access at the edge — there is no shared password. Automation uses a Cloudflare Access service token (`CF-Access-Client-Id` / `CF-Access-Client-Secret` headers).

## Services (native systemd)

| Service | Unit | Port | Description |
|---------|------|------|-------------|
| Core | aetheris-core | 127.0.0.1:8080 | Rust Axum API server |
| Tunnel | cloudflared | — | Cloudflare Tunnel + Access (edge TLS/auth) |
| Ollama | ollama | 127.0.0.1:11434 | Local LLM inference (qwen2.5:7b default) |
| Orchestrator | (llmvm profile) | 127.0.0.1:9090 | RAG pipeline, KG (optional) |

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
├── infra/systemd/          # systemd units (native deploy)
├── config/core.env.example # Non-secret core environment template
└── scripts/                # install-native.sh, bootstrap.sh, verification.sh, killswitch.sh
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
- **Encrypted tunnel**: Cloudflare Tunnel provides TLS from browser to the loopback core
- **Access**: Public hostnames gated by Cloudflare Access (identity-gated; no HTTP Basic Auth)
- **WAL audit**: All file and system operations logged to append-only WAL

## Documentation

See `docs/README.md` for the full documentation index. Key documents:

- [Native Deployment Guide](docs/DEPLOY_NATIVE.md)
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
