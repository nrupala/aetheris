# Getting Started with Aetheris

## Prerequisites

- **Rust 1.75+** — [Install Rust](https://rustup.rs/)
- **Docker & Docker Compose** — [Install Docker](https://docs.docker.com/get-docker/)
- **Linux/macOS/Windows** — Platform-neutral support

## Quick Install

### One-liner (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/nrupala/aetheris/main/install.sh | bash
```

### Manual Install

```bash
# Clone the repository
git clone https://github.com/nrupala/aetheris.git
cd aetheris

# Build from source
cd core && cargo build --release && cd ..

# Or use pre-built binaries from GitHub Releases
```

## Running with Docker

```bash
# Start core services only (no LLMVM dependencies)
docker compose up -d

# Start with LLMVM services (requires ../LLMVM directory)
docker compose --profile llmvm up -d

# Check status
curl http://localhost:8080/status
```

## Configuration

Create a `.env` file:

```env
VAULT_PATH=/data/vault
AI_ENDPOINT=http://host.docker.internal:1234
OPA_ENDPOINT=http://opa:8181
PORT=8080
CLOUDFLARE_TUNNEL_TOKEN=your-token-here
DEV_PASSWORD=your-dev-password
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Aetheris Core                           │
├─────────────┬─────────────┬─────────────┬──────────────────┤
│   Gateway   │   Storage   │   Identity  │   AI Policy      │
│   (Axum)   │   (ZFS)     │   (OpenID)  │   (OPA Bridge)   │
└─────────────┴─────────────┴─────────────┴──────────────────┘
```

## Services

| Service | Port | Description |
|---------|------|-------------|
| Aetheris Core | 8080 | Main API server |
| OPA Gateway | 8181 | Policy engine |
| Vector DB | 8000 | ChromaDB semantic store |
| VictoriaMetrics | 8428 | Time-series metrics |
| Nginx | 9080/9443 | Reverse proxy |

## Development

```bash
# Run tests
cargo test --all

# Run with hot reload
cargo watch -x run

# Format and lint
cargo fmt && cargo clippy -- -D warnings
```

## Platform Binaries

Pre-built binaries are available for:

- `linux/amd64` (GNU and musl)
- `linux/arm64` (GNU and musl)
- `windows/amd64`
- `macOS/amd64`
- `macOS/arm64`

Download from [GitHub Releases](https://github.com/nrupala/aetheris/releases).
