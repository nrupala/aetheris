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

### Docker
```bash
# Build and start
docker compose build                       # Build all images
docker compose up -d                       # Start all services
docker compose up -d --profile llmvm       # Start with LLMVM (agent orchestrator)

# Manage
docker compose ps                          # List running services
docker compose logs -f                     # Follow all logs
docker compose logs -f core               # Follow core service logs
docker compose restart core               # Restart core only

# Stop
docker compose down                        # Stop and remove containers
docker compose down -v                     # ⚠️ Also removes volumes (data loss!)
docker compose down --rmi all              # Remove images too

# Clean rebuild
docker compose build --no-cache core       # Rebuild core from scratch
docker compose up -d --force-recreate      # Force recreate containers
```

## Service Health

### Health Checks
```bash
# Core API health
curl -u dev_user:BCjfTYIIjMASFGVM https://dev.nrupalakolkar.com/api/health

# AI service
curl -u ai_user:BCjfTYIIjMASFGVM https://ai.nrupalakolkar.com/v1/models

# RAG service
curl -u rag_user:BCjfTYIIjMASFGVM https://rag.nrupalakolkar.com/health

# Full verification
./scripts/verification.sh
```

## API Usage

### AI Chat
```bash
curl -u ai_user:BCjfTYIIjMASFGVM \
  https://ai.nrupalakolkar.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"qwen2.5:14b","messages":[{"role":"user","content":"Hello"}],"max_tokens":100}'
```

### RAG Query
```bash
curl -u rag_user:BCjfTYIIjMASFGVM \
  https://rag.nrupalakolkar.com/query \
  -H "Content-Type: application/json" \
  -d '{"query":"What is the project structure?","use_rag":true,"top_k":5,"threshold":0.7}'
```

### RAG Upload
```bash
curl -u rag_user:BCjfTYIIjMASFGVM \
  https://rag.nrupalakolkar.com/ingest/file \
  -F "file=@document.pdf"
```

### Workflow
```bash
curl -u dev_user:BCjfTYIIjMASFGVM \
  https://agents.nrupalakolkar.com/api/workflow/run \
  -H "Content-Type: application/json" \
  -d '{"task":"Analyze the architecture","max_iterations":3,"use_reasoning":true}'
```

### Agent Status
```bash
curl -u dev_user:BCjfTYIIjMASFGVM https://agents.nrupalakolkar.com/api/agents/status
```

### Orchestrator State
```bash
curl -u dev_user:BCjfTYIIjMASFGVM https://agents.nrupalakolkar.com/api/orchestrator/state
curl -u dev_user:BCjfTYIIjMASFGVM https://agents.nrupalakolkar.com/api/orchestrator/forecast
```

### MCP Tools
```bash
curl -u dev_user:BCjfTYIIjMASFGVM https://agents.nrupalakolkar.com/api/mcp/tools
```

### A2A Messages
```bash
curl -u dev_user:BCjfTYIIjMASFGVM https://agents.nrupalakolkar.com/api/a2a/messages
```

### Dev Sandbox
```bash
# System logs
curl -u dev_user:BCjfTYIIjMASFGVM https://dev.nrupalakolkar.com/api/dev/logs

# Configuration
curl -u dev_user:BCjfTYIIjMASFGVM https://dev.nrupalakolkar.com/api/dev/config

# Metrics
curl -u dev_user:BCjfTYIIjMASFGVM https://dev.nrupalakolkar.com/api/dev/metrics
```

### Knowledge Graph
```bash
# Stats
curl -u rag_user:BCjfTYIIjMASFGVM https://rag.nrupalakolkar.com/api/knowledge-graph/stats

# Entities
curl -u rag_user:BCjfTYIIjMASFGVM "https://rag.nrupalakolkar.com/api/knowledge-graph/entities?entity_type=concept&limit=50"

# Relations
curl -u rag_user:BCjfTYIIjMASFGVM https://rag.nrupalakolkar.com/api/knowledge-graph/relations
```

### Circuit Breakers
```bash
curl -u dev_user:BCjfTYIIjMASFGVM https://dev.nrupalakolkar.com/api/coordinator/circuits
```

## Tunnel & Proxy

### Cloudflare Tunnel
```bash
# Check tunnel status
cloudflared tunnel list

# Restart tunnel
cloudflared tunnel restart 267e8c28-...

# View tunnel logs
cloudflared tunnel logs 267e8c28-...
```

### Nginx
```bash
# Test configuration
nginx -t

# Reload config
nginx -s reload
```

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

### Clean Reset
```bash
# Full reset (preserves data)
docker compose down
docker compose up -d

# Hard reset (⚠️ data loss)
docker compose down -v
docker compose build --no-cache
docker compose up -d
```
