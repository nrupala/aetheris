# Aetheris — Role-Based Guides

Guides organized by persona, following the AppDocStudio role-based documentation standard.

---

## 👤 User Guide

*You interact with Aetheris through web interfaces for AI chat, document Q&A, and agent workflows.*

### Access Points
| Service | URL | Auth |
|---------|-----|------|
| AI Chat | `https://ai.nrupalakolkar.com` | Cloudflare Access |
| RAG Q&A | `https://rag.nrupalakolkar.com` | Cloudflare Access |
| Agent Dashboard | `https://agents.nrupalakolkar.com` | Cloudflare Access |
| Dev Sandbox | `https://dev.nrupalakolkar.com` | Cloudflare Access |

**Auth:** Cloudflare Access (identity-gated at the edge — no shared password).

### Common Tasks

**Chat with AI:** Open `https://ai.nrupalakolkar.com`, sign in via Cloudflare Access, type your question. The UI uses the `phi4-mini` / `qwen3:8b` models.

**Query documents:** Open `https://rag.nrupalakolkar.com`, upload documents in the Documents tab, then ask questions in the Ask tab.

**Run agent workflow:** Open `https://agents.nrupalakolkar.com`, go to the Workflow tab, describe your task, and click Run. The Planner→Researcher→Coder→Reviewer pipeline executes automatically.

**Test APIs:** Open `https://dev.nrupalakolkar.com`, use the API Console tab to execute any endpoint.

### Tips
- All four hostnames are gated by the same Cloudflare Access application
- AI chat maintains conversation history automatically
- RAG supports PDF, TXT, MD, HTML, JSON, CSV, and code files (50MB max)
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
├── infra/systemd/          # systemd units (native deploy)
├── config/core.env.example # Non-secret core environment template
├── scripts/install-native.sh  # Native (no-Docker) installer
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
- Ollama is at `127.0.0.1:11434` (loopback), NOT LMStudio at `:1234`
- Only one model: `qwen2.5:7b`
- Auth is Cloudflare Access (identity-gated) — no static credentials in code or config
- `--profile llmvm` gates the Python orchestrator proxy

---

## 🔧 Maintainer Guide

*You deploy, upgrade, and ensure the reliability of Aetheris in production.*

### Production Stack
```
Browser → Cloudflare Access (edge auth) → Cloudflare Tunnel → aetheris-core (127.0.0.1:8080)
  └── ai/rag/dev/oracle/agents panels + agent API (all core; agents.* routes to :8080)
      └── Ollama (127.0.0.1:11434)
```
The core runs as a native `systemd` service (`aetheris-core`). There is no nginx —
cloudflared points straight at the loopback core. `agents.*` routes to core (`:8080`), not a
separate `:9090` service; `:9090` is the separate mgmt service (excluded from `CF_ACCESS_AUD`).

### Deployment
```bash
# Install / update (idempotent)
sudo scripts/install-native.sh

# Verify
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://dev.nrupalakolkar.com/api/health
./scripts/verification.sh
```
See `docs/DEPLOY_NATIVE.md` for the full runbook.

### Upgrade Procedure
1. Pull latest code: `git pull`
2. Reinstall: `sudo scripts/install-native.sh` (rebuilds the binary, restarts the service)
3. Verify health
4. For major upgrades, back up the vault first

### Monitoring
- **Health endpoint:** `GET /api/health` — agent count, task count, cross-system status
- **Dev logs:** `GET /api/dev/logs` — WAL-backed audit trail
- **Metrics:** `GET /api/dev/metrics` — service health, uptime
- **Circuit breakers:** `GET /api/coordinator/circuits` — engine health states
- **Service logs:** `journalctl -u aetheris-core -f`

### Backup
```bash
# RAG data + WAL (audit log)
cp -r /data/vault/ ./backups/vault/

# Config
cp -r /etc/aetheris/ ./backups/config/
```

### Emergency Procedures
1. **Kill switch:** `./scripts/killswitch.sh` — stops everything, shreds secrets
2. **Service failure:** Check logs: `journalctl -u aetheris-core -n 100`
3. **Ollama failure:** Restart: `systemctl restart ollama`
4. **Tunnel failure:** Check: `cloudflared tunnel list` — restart: `cloudflared tunnel restart <id>`

---

## 🏛️ Regulator Guide

*You audit Aetheris for compliance, security standards, and data protection.*

### Security Architecture
- **Encryption at rest:** ZFS with AES-256-GCM
- **Encryption in transit:** Cloudflare Tunnel (TLS) + WireGuard mesh (Curve25519)
- **Access control:** Cloudflare Access (identity-gated at the edge) + OPA policy engine for fine-grained authorization
- **Audit trail:** WAL (Write-Ahead Log) records all operations with sequence numbers
- **Session isolation:** All user sessions are isolated with no shared state

### Audit Points
1. **WAL integrity:** Verify sequence numbers are monotonic and unbroken
2. **OPA policies:** Review `default deny` enforcement and role definitions
3. **Access logs:** Cloudflare Access logs capture every authenticated request with identity
4. **File integrity:** ZFS snapshots provide point-in-time evidence
5. **Data retention:** Uploaded source files persist in the vault; the audit WAL under `/data/vault/wal` is the retention record
6. **Emergency protocol:** Kill switch procedure documented and tested

### Compliance Checklist
- [ ] WAL replay produces valid sequence without gaps
- [ ] OPA default deny returns false for unauthorized actions
- [ ] All API endpoints require authentication (Cloudflare Access)
- [ ] No plaintext secrets in source code or config
- [ ] TLS termination at Cloudflare edge
- [ ] ZFS native encryption enabled on all datasets
- [ ] Audit logs written for every file operation

---

## 👑 Admin Guide

*You manage users, credentials, and access policies.*

### User Management
Aetheris uses **Cloudflare Access** at the edge for authentication. Access is granted by
policy in the Cloudflare Zero Trust dashboard rather than per-service accounts:

| Access | Hostname | Notes |
|--------|----------|-------|
| Identity policy | `ai.nrupalakolkar.com` | AI chat interface |
| Identity policy | `rag.nrupalakolkar.com` | RAG document Q&A |
| Identity policy | `agents.nrupalakolkar.com`, `dev.nrupalakolkar.com` | Dashboard + sandbox |

Grant or revoke access by editing the Access application's policy (allowed emails /
groups). For scripted/API access, mint a **service token**.

### Managing Credentials
There are no static passwords. Interactive users authenticate with their identity;
automation uses a Cloudflare Access **service token** passed as headers:
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" \
     -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
     https://core.nrupalakolkar.com/api/health
```
Rotate the token from the Cloudflare Zero Trust dashboard; nothing on the box changes.

### Access Logs
Authentication events are in the Cloudflare Access logs (Zero Trust dashboard → Logs →
Access). Core request logs:
```bash
journalctl -u aetheris-core | grep "auth"
```

### OPA Policy Management
OPA policies are in `core/policies/`. To modify:
1. Edit Rego policy files
2. Test locally: `opa eval --data policies/ "data.aetheris.allow"`
3. Restart core: `systemctl restart aetheris-core`

---

## ⚙️ Operations Guide

*You run, monitor, and scale the Aetheris infrastructure.*

### Service Architecture
```
Native systemd services (127.0.0.1 loopback):
├── aetheris-core (:8080)   — Rust Axum API gateway + RAG pipeline + vector store
├── cloudflared             — Cloudflare Tunnel + Access (edge TLS/auth)
└── ollama (:11434)         — LLM inference (phi4-mini, qwen3:8b) + embeddings (nomic-embed-text)
```
RAG and the knowledge graph run inside `aetheris-core` (SQLite in `/data/vault`) — no separate RAG service, Chroma, or Python orchestrator is required on the native deploy.

### Routine Maintenance
```bash
# Daily
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://dev.nrupalakolkar.com/api/health

# Weekly
journalctl -u aetheris-core --since "1 week ago" > /var/log/aetheris/core-weekly.log

# Monthly
git pull && sudo scripts/install-native.sh
```

### Scaling Considerations
- **Memory:** qwen3:8b requires ~8GB RAM; phi4-mini ~3GB. The box has 24GB total (see `/api/health` → `total_memory_mb`)
- **Storage:** RAG data grows with uploaded documents; monitor the `/data/vault` directory
- **Concurrency:** Axum handles concurrent requests efficiently; monitor WAL write throughput
- **Network:** Cloudflare Tunnel handles up to 100Mbps; upgrade plan for higher throughput

### Troubleshooting

**502 Bad Gateway:**
```bash
journalctl -u aetheris-core -n 100
# Likely: Ollama not reachable — check `systemctl status ollama`
```

**Ollama model not found:**
```bash
ollama pull qwen2.5:7b
```

**Agent workflow fails:**
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://dev.nrupalakolkar.com/api/agents/status
# Check agent states — executing vs failed
```

**Cloudflare Tunnel disconnected:**
```bash
cloudflared tunnel list
cloudflared tunnel restart <tunnel-id>
```
