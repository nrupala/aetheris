# Aetheris — Role-Based Guides

Guides organized by persona, following the AppDocStudio role-based documentation standard.

---

## 👤 User Guide

*You interact with Aetheris through web interfaces for AI chat, document Q&A, and agent workflows.*

### Access Points
| Service | URL | Auth |
|---------|-----|------|
| AI Chat | `https://ai.nrupalakolkar.com` | `ai_user` |
| RAG Q&A | `https://rag.nrupalakolkar.com` | `rag_user` |
| Agent Dashboard | `https://agents.nrupalakolkar.com` | Shared creds |
| Dev Sandbox | `https://dev.nrupalakolkar.com` | Shared creds |

**Shared Password:** `BCjfTYIIjMASFGVM`

### Common Tasks

**Chat with AI:** Open `https://ai.nrupalakolkar.com`, enter credentials, type your question. The UI uses the `qwen2.5:14b` model.

**Query documents:** Open `https://rag.nrupalakolkar.com`, upload documents in the Documents tab, then ask questions in the Ask tab.

**Run agent workflow:** Open `https://agents.nrupalakolkar.com`, go to the Workflow tab, describe your task, and click Run. The Planner→Researcher→Coder→Reviewer pipeline executes automatically.

**Test APIs:** Open `https://dev.nrupalakolkar.com`, use the API Console tab to execute any endpoint.

### Tips
- All four subdomains share the same password
- AI chat maintains conversation history automatically
- RAG supports PDF, TXT, MD, HTML, JSON, CSV files (50MB max)
- Agent workflows show step-by-step execution with duration
- The Dev Sandbox shows live system logs and service metrics

---

## 🛠️ Developer Guide

*You build, extend, and maintain the Aetheris codebase.*

### Project Structure
```
aetheris/
├── core/                    # Rust source code
│   ├── src/
│   │   ├── main.rs         # Entry point, Axum server (847 lines)
│   │   ├── lib.rs          # Module exports
│   │   ├── bridge.rs       # Trait abstractions (4 traits)
│   │   ├── implementation.rs  # OllamaBridge + OpaBridge
│   │   ├── sync.rs         # File sync handlers
│   │   ├── agents/         # Agent trait + 4 agents
│   │   ├── a2a.rs          # Agent-to-Agent messaging
│   │   ├── mcp.rs          # MCP protocol tools
│   │   ├── proxy.rs        # Reverse proxy to Python
│   │   └── wal.rs          # Write-ahead log
│   └── Cargo.toml
├── web/                    # HTML subdomain UIs
├── compose.yaml            # Docker Compose
└── scripts/               # Deployment scripts
```

### Development Workflow
```bash
# Check compilation
cargo check

# Run tests
cargo test

# Format and lint
cargo fmt --all
cargo clippy -- -D warnings

# Build with LLMVM profile
cargo build --profile llmvm
```

### Key Conventions
- **Bridge traits** are the abstraction boundary — always implement `ModelBridge` or `SecurityBridge` for new AI/security backends
- **All handlers** must return Send futures — never hold `MutexGuard` across `.await`
- **WAL** is append-only — new entry types go in `WalEntryType` enum
- **Agents** implement the `Agent` trait and register in the factory

### Adding a New Endpoint
1. Add handler function in appropriate module
2. Register route in `main.rs` router
3. Add tests in `core/tests/`
4. Document in `docs/api-reference/README.md`

### Common Gotchas
- Ollama is at `:11434`, NOT LMStudio at `:1234`
- Only one model: `qwen2.5:14b`
- Password for all users: `BCjfTYIIjMASFGVM`
- `--profile llmvm` gates the Python orchestrator proxy

---

## 🔧 Maintainer Guide

*You deploy, upgrade, and ensure the reliability of Aetheris in production.*

### Production Stack
```
Browser → Cloudflare Tunnel → Nginx (auth) → Rust Core (:8080)
  └── optionally → Python Orchestrator (:9090)
                     └── Ollama (:11434)
```

### Deployment
```bash
# Full stack
docker compose build
docker compose up -d

# With LLMVM (agent orchestrator)
docker compose --profile llmvm up -d

# Verify
curl -u dev_user:BCjfTYIIjMASFGVM https://dev.nrupalakolkar.com/api/health
./scripts/verification.sh
```

### Upgrade Procedure
1. Pull latest code: `git pull`
2. Rebuild: `docker compose build core`
3. Restart: `docker compose up -d --force-recreate core`
4. Verify health
5. For major upgrades, back up volumes first

### Monitoring
- **Health endpoint:** `GET /api/health` — agent count, task count, cross-system status
- **Dev logs:** `GET /api/dev/logs` — WAL-backed audit trail
- **Metrics:** `GET /api/dev/metrics` — service health, containers, uptime
- **Circuit breakers:** `GET /api/coordinator/circuits` — engine health states

### Backup
```bash
# RAG data
docker cp <rag-container>:/app/data ./backups/rag/

# Config
cp -r /etc/aetheris/ ./backups/config/

# WAL
cp -r /path/to/vault/wal/ ./backups/wal/
```

### Emergency Procedures
1. **Kill switch:** `./scripts/killswitch.sh` — stops everything, shreds secrets
2. **Service failure:** Check logs: `docker compose logs -f core`
3. **Ollama failure:** Restart: `docker compose restart ollama`
4. **Tunnel failure:** Check: `cloudflared tunnel list` — restart: `cloudflared tunnel restart <id>`

---

## 🏛️ Regulator Guide

*You audit Aetheris for compliance, security standards, and data protection.*

### Security Architecture
- **Encryption at rest:** ZFS with AES-256-GCM
- **Encryption in transit:** Cloudflare Tunnel (TLS) + WireGuard mesh (Curve25519)
- **Access control:** HTTP Basic Auth at Nginx + OPA policy engine for fine-grained authorization
- **Audit trail:** WAL (Write-Ahead Log) records all operations with sequence numbers
- **Session isolation:** All user sessions are isolated with no shared state

### Audit Points
1. **WAL integrity:** Verify sequence numbers are monotonic and unbroken
2. **OPA policies:** Review `default deny` enforcement and role definitions
3. **Access logs:** Nginx access logs capture all requests with timestamps
4. **File integrity:** ZFS snapshots provide point-in-time evidence
5. **Data retention:** RAG pipeline auto-cleans temporary files (1h-7d TTL)
6. **Emergency protocol:** Kill switch procedure documented and tested

### Compliance Checklist
- [ ] WAL replay produces valid sequence without gaps
- [ ] OPA default deny returns false for unauthorized actions
- [ ] All API endpoints require authentication (HTTP Basic Auth)
- [ ] No plaintext secrets in source code or config
- [ ] TLS termination at Cloudflare edge
- [ ] ZFS native encryption enabled on all datasets
- [ ] Audit logs written for every file operation

---

## 👑 Admin Guide

*You manage users, credentials, and access policies.*

### User Management
Aetheris uses HTTP Basic Auth at the Nginx proxy layer. There are three user accounts:

| User | Subdomain Access | Notes |
|------|-----------------|-------|
| `ai_user` | `ai.nrupalakolkar.com` | AI chat interface |
| `rag_user` | `rag.nrupalakolkar.com` | RAG document Q&A |
| `dev_user` | `agents.nrupalakolkar.com`, `dev.nrupalakolkar.com` | Dashboard + sandbox |

All users share the same password: `BCjfTYIIjMASFGVM`

### Managing Credentials
Credentials are configured via environment variables in Docker Compose:
```yaml
NGINX_BASIC_AUTH: "ai_user:$2y$10$...;rag_user:$2y$10$...;dev_user:$2y$10$..."
```
Use `htpasswd` to generate new password hashes:
```bash
htpasswd -nbB ai_user "new-password"
```

### Access Logs
Nginx logs are available at:
```bash
docker compose logs nginx | grep "auth"
```

### OPA Policy Management
OPA policies are in `core/policies/`. To modify:
1. Edit Rego policy files
2. Test locally: `opa eval --data policies/ "data.aetheris.allow"`
3. Restart core: `docker compose restart core`

---

## ⚙️ Operations Guide

*You run, monitor, and scale the Aetheris infrastructure.*

### Service Architecture
```
Docker Services:
├── core (:8080)          — Rust Axum API gateway
├── nginx (:443)          — SSL termination, auth, reverse proxy
├── ollama (:11434)       — LLM inference (qwen2.5:14b)
├── orchestrator (:9090)  — RAG pipeline, KG (optional, LLMVM profile)
├── chroma                — Vector database (optional)
├── opa (:8181)           — Policy engine (optional)
├── victoria-metrics (:8428)  — Metrics (optional)
└── grafana (:3000)       — Visualization (optional)
```

### Routine Maintenance
```bash
# Daily
curl -u dev_user:BCjfTYIIjMASFGVM https://dev.nrupalakolkar.com/api/health

# Weekly
docker compose logs --tail=100 core > /var/log/aetheris/core-weekly.log
docker system prune -f

# Monthly
docker compose build --no-cache core
```

### Scaling Considerations
- **Memory:** qwen2.5:14b requires ~8GB RAM for the model alone
- **Storage:** RAG data grows with uploaded documents; monitor `rag_data` volume
- **Concurrency:** Axum handles concurrent requests efficiently; monitor WAL write throughput
- **Network:** Cloudflare Tunnel handles up to 100Mbps; upgrade plan for higher throughput

### Troubleshooting

**502 Bad Gateway:**
```bash
docker compose logs core
# Likely: Ollama not reachable — check docker compose logs ollama
```

**Ollama model not found:**
```bash
docker compose exec ollama ollama pull qwen2.5:14b
```

**Agent workflow fails:**
```bash
curl -u dev_user:BCjfTYIIjMASFGVM https://dev.nrupalakolkar.com/api/agents/status
# Check agent states — executing vs failed
```

**Cloudflare Tunnel disconnected:**
```bash
cloudflared tunnel list
cloudflared tunnel restart <tunnel-id>
```
