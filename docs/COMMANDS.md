# Aetheris — Common Commands

## Build & Development

### Rust Core
```bash
cd core

# Build
cargo build                                # Debug build
cargo build --release                      # Release build
cargo build --profile llmvm                # LLMVM profile (with Python orchestrator)

# Check (fast compile-time verification)
cargo check                                # Verify compilation without producing binary

# Test
cargo test                                 # All tests
cargo test -- --nocapture                  # Tests with stdout visible
cargo test --test '*'                      # Integration tests only

# Lint & Format
cargo fmt --all                            # Format all Rust code
cargo clippy -- -D warnings                # Lint with warnings as errors

# Run locally
cargo run                                  # Run debug build
cargo run --release                        # Run release build
./target/release/aetheris                  # Run compiled binary
```

### Native service (systemd)
The core runs as a native systemd service — no Docker. See `docs/DEPLOY_NATIVE.md`.
```bash
# Install / update (idempotent; builds the static musl binary, installs unit + env)
sudo scripts/install-native.sh

# Manage
systemctl status aetheris-core             # Service status
systemctl restart aetheris-core            # Restart core
systemctl enable --now aetheris-core       # Enable + start
journalctl -u aetheris-core -f             # Follow core logs
journalctl -u aetheris-core -n 100         # Last 100 log lines
```

## Service Health

### Health Checks
Auth is via Cloudflare Access — API calls carry a service token in headers (no static
credentials). The token values live in your environment / secret store.
```bash
# Core API health
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://dev.nrupalakolkar.com/api/health

# AI service
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://ai.nrupalakolkar.com/v1/models

# RAG service
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://rag.nrupalakolkar.com/health

# Full verification
./scripts/verification.sh
```

## API Usage

### AI Chat
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
  https://ai.nrupalakolkar.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"qwen2.5:7b","messages":[{"role":"user","content":"Hello"}],"max_tokens":100}'
```

### RAG Query
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
  https://rag.nrupalakolkar.com/query \
  -H "Content-Type: application/json" \
  -d '{"query":"What is the project structure?","reasoning_enabled":false,"top_k":5,"reranker_enabled":false}'
```

### RAG Config
```bash
# Read current config
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
  https://rag.nrupalakolkar.com/config

# Partial update (fields not sent keep their current values)
curl -X PUT -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
  https://rag.nrupalakolkar.com/config \
  -H "Content-Type: application/json" \
  -d '{"query_model":"phi4-mini","top_k":5}'
```

### RAG Upload
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
  https://rag.nrupalakolkar.com/ingest/file \
  -F "file=@document.pdf"
```

### Workflow
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
  https://agents.nrupalakolkar.com/api/workflow/run \
  -H "Content-Type: application/json" \
  -d '{"task":"Analyze the architecture","max_iterations":3,"use_reasoning":true}'
```

### Agent Status
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://agents.nrupalakolkar.com/api/agents/status
```

### Orchestrator State
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://agents.nrupalakolkar.com/api/orchestrator/state
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://agents.nrupalakolkar.com/api/orchestrator/forecast
```

### MCP Tools
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://agents.nrupalakolkar.com/api/mcp/tools
```

### A2A Messages
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://agents.nrupalakolkar.com/api/a2a/messages
```

### Dev Sandbox
```bash
# System logs
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://dev.nrupalakolkar.com/api/dev/logs

# Configuration
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://dev.nrupalakolkar.com/api/dev/config

# Metrics
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://dev.nrupalakolkar.com/api/dev/metrics
```

### Knowledge Graph
```bash
# Stats
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://rag.nrupalakolkar.com/api/knowledge-graph/stats

# Entities
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" "https://rag.nrupalakolkar.com/api/knowledge-graph/entities?entity_type=concept&limit=50"

# Relations
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://rag.nrupalakolkar.com/api/knowledge-graph/relations
```

### Circuit Breakers
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://dev.nrupalakolkar.com/api/coordinator/circuits
```

## Tunnel & Access

### Cloudflare Tunnel
cloudflared points straight at the loopback core (`http://127.0.0.1:8080`); there is no
nginx. The public hostname `core.nrupalakolkar.com` is gated by Cloudflare Access.
```bash
# Check tunnel status
cloudflared tunnel list

# Restart tunnel
cloudflared tunnel restart <tunnel-id>

# View tunnel logs
cloudflared tunnel logs <tunnel-id>

# Restart the tunnel service
systemctl restart cloudflared
```

### Cloudflare Access
Auth is enforced at the edge by Cloudflare Access (no HTTP Basic Auth). Manage the
Access application and its policies / service tokens from the Cloudflare Zero Trust
dashboard. For scripted access, mint a service token and pass it as the
`CF-Access-Client-Id` / `CF-Access-Client-Secret` headers shown above.

## Git Workflow

```bash
# Create feature branch
git checkout -b feature/my-feature

# Commit with convention
git add <files>
git commit -m "feat: add widget endpoint"
git commit -m "fix: correct MutexGuard usage in handler"
git commit -m "docs: update API reference"

# Push and create PR
git push -u origin feature/my-feature
gh pr create --title "feat: widget endpoint" --body "Implements GET /api/widget"

# Sync with main
git checkout main
git pull
git merge feature/my-feature
```

## Emergency

### Kill Switch
```bash
# Emergency shutdown
./scripts/killswitch.sh

# Verify no services remain
curl -s https://dev.nrupalakolkar.com/api/health || echo "Services offline"
```

### Clean Restart
```bash
# Restart the native core service
systemctl restart aetheris-core

# Full reinstall (rebuilds binary, re-installs unit; preserves /etc/aetheris/core.env)
sudo scripts/install-native.sh
systemctl status aetheris-core
```
