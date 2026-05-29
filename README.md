# Aetheris — Sovereign AI-Native Personal Cloud

Zero-trust personal cloud with AI-powered policy enforcement, secure storage, and local AI agents. Built with Rust, OPA, ZFS, and WireGuard.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                     Aetheris Core                         │
├────────────┬───────────┬──────────┬─────────────────────┤
│  Gateway   │  Storage  │ Identity │   AI Policy          │
│  (Axum)   │  (Vault)  │ (OPA)    │   (LMStudio/Ollama)  │
└────────────┴───────────┴──────────┴─────────────────────┘
```

## Quick Start

### Prerequisites
- Rust 1.86+
- Docker + Docker Compose v2
- A Linux server (production) or local machine (development)

### Local Development

```bash
# Build the core service
cd core && cargo build --release

# Run tests
cargo test --lib

# Start full stack with Docker Compose
docker compose up -d

# Verify health
curl http://localhost:8080/status
```

### Production Deployment

```bash
# 1. Clone on server
git clone https://github.com/your-org/aetheris.git /opt/aetheris
cd /opt/aetheris

# 2. Configure environment
cp .env.example .env
# Edit .env with your secrets

# 3. Bootstrap system
sudo bash scripts/bootstrap.sh

# 4. Start all services
docker compose up -d

# 5. Verify deployment
bash scripts/verification.sh
```

## Services

| Service | Container | Port | Description |
|---------|-----------|------|-------------|
| Core | aetheris_core | 8080 | Rust API server |
| Nginx | llmvm_nginx | 9080/9443 | Reverse proxy + auth |
| OPA | aetheris_opa | 8181 | Policy engine |
| Vector DB | aetheris_vectors | 8000 | ChromaDB |
| Metrics | aetheris_stats | 8428 | VictoriaMetrics |
| WireGuard | aetheris_mesh | 51820 | VPN mesh |

## Subdomains

| Domain | Purpose | Auth |
|--------|---------|------|
| nrupalakolkar.com | Landing page | No |
| dev.nrupalakolkar.com | Dev sandbox & API console | Basic |
| ai.nrupalakolkar.com | AI chat interface | Basic |
| rag.nrupalakolkar.com | Document search | Basic |
| agents.nrupalakolkar.com | Multi-agent dashboard | Basic |
| oracle.nrupalakolkar.com | Raw model access | Basic |
| git.nrupalakolkar.com | Gitea repos | No |
| ci.nrupalakolkar.com | Woodpecker CI | No |

All credentials for auth-protected subdomains are in `nginx/.htpasswd`.
Default credential: `dev_user` / `BCjfTYIIjMASFGVM`

## Development

### Project Structure

```
aetheris/
├── core/              # Rust source code
│   ├── src/
│   │   ├── main.rs   # Axum server + API routes
│   │   ├── lib.rs    # Module exports
│   │   ├── connector.rs  # OPA + AI integration
│   │   ├── config.rs     # Configuration
│   │   ├── metrics.rs    # Prometheus metrics
│   │   ├── wal.rs        # Write-ahead log
│   │   └── watcher.rs    # Security watcher
│   └── ui/           # Embedded dashboard HTML
├── nginx/            # Reverse proxy configs
├── web/              # Static HTML pages
├── scripts/          # Deployment tools
├── config/           # OPA policies + port registry
└── .github/workflows/  # CI/CD pipelines
```

### Making Changes

1. Create a feature branch
2. Build and test locally: `cd core && cargo build && cargo test --lib`
3. For nginx changes, edit `nginx/default.conf` and `nginx/ssl.conf`
4. Run verification: `bash scripts/verification.sh` (from Docker host)
5. Commit and push to main — CI/CD auto-deploys

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | System health |
| `/status` | GET | Component status |
| `/v1/models` | GET | Available AI models |
| `/query` | POST | RAG document query |
| `/sources` | GET | List document sources |
| `/ingest/file` | POST | Upload document |
| `/agents/status` | GET | Agent status |
| `/dev/logs` | GET | System logs |
| `/dev/config` | GET | Config files |
| `/dev/metrics` | GET | Container metrics |

Access via dev subdomain: `GET /api/health` (nginx rewrites `/api/` prefix)

## Security

- **Zero-trust**: Every request evaluated by OPA policies
- **Encrypted vault**: AES-256-GCM at rest via ZFS
- **WireGuard mesh**: All inter-node traffic encrypted
- **Basic auth**: Protected subdomains via nginx htpasswd
- **No JS on root**: Server-rendered HTML, CSP headers

## Emergency Procedures

```bash
# Run health checks
bash scripts/verification.sh

# Emergency shutdown (irreversible!)
sudo bash scripts/killswitch.sh

# Dry run first
sudo bash scripts/killswitch.sh --dry-run
```

## Troubleshooting

### Container won't start
```bash
# Check logs
docker compose logs aetheris-core
docker compose logs llmvm_nginx

# Rebuild and restart
docker compose build aetheris-core
docker compose up -d --force-recreate aetheris-core
```

### Config not updating
```bash
# Nginx configs are bind-mounted. Edit host files, then:
docker compose exec llmvm_nginx nginx -s reload

# Or recreate container to pick up bind mount
docker compose up -d --force-recreate llmvm_nginx
```

### All endpoints return 502
```bash
# Core is likely down. Restart it:
docker compose up -d --force-recreate aetheris-core
```
