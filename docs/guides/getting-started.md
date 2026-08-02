# Getting Started with Aetheris

## Prerequisites

- **Rust 1.75+** — [Install Rust](https://rustup.rs/)
- **Ollama** — listening loopback-only at `127.0.0.1:11434`, with `qwen2.5:7b` (generation) and `nomic-embed-text` (embeddings) pulled
- **systemd** and root/sudo on a Linux host (`x86_64` or `aarch64`)
- **cloudflared** — for public ingress via Cloudflare Tunnel (optional for local-only use)
- **Linux/macOS/Windows** — Platform-neutral support for building the binary

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

## Running (native systemd, no Docker)

Aetheris Core runs directly on the host under `systemd` — there is no Docker, no
compose, and no nginx. Install and start it with the native installer:

```bash
# Build, install, and enable the aetheris-core systemd service
sudo scripts/install-native.sh

# Check status (core listens loopback-only on 127.0.0.1:8080)
systemctl is-active aetheris-core
curl -sf http://127.0.0.1:8080/status
```

See [`../DEPLOY_NATIVE.md`](../DEPLOY_NATIVE.md) for the full native deployment,
cloudflared ingress, and Cloudflare Access setup.

## Configuration

The installer writes `/etc/aetheris/core.env` from `config/core.env.example` (it never
overwrites an existing file). Key settings — no secrets:

```env
VAULT_PATH=/data/vault
AI_ENDPOINT=http://127.0.0.1:11434
AETHERIS_FALLBACK_MODEL=qwen2.5:7b
AETHERIS_EMBED_FALLBACK_MODEL=nomic-embed-text
OPA_ENDPOINT=http://127.0.0.1:8181
PORT=8080
```

Public ingress and auth are handled by cloudflared + Cloudflare Access (no
`CLOUDFLARE_TUNNEL_TOKEN` env, no HTTP Basic Auth) — see `../DEPLOY_NATIVE.md`.

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

Aetheris Core runs as a single native `systemd` service and reaches its dependencies
on the loopback interface. Public access is via cloudflared, not a local reverse proxy.

| Service | Address | Description |
|---------|---------|-------------|
| Aetheris Core | 127.0.0.1:8080 | Main API server (systemd `aetheris-core`, loopback-only) |
| Ollama | 127.0.0.1:11434 | Local AI: generation + embeddings (loopback-only) |
| OPA Gateway | 127.0.0.1:8181 | Policy engine |
| Vector DB | 8000 | ChromaDB semantic store (optional) |
| VictoriaMetrics | 8428 | Time-series metrics (optional) |
| cloudflared | — | Cloudflare Tunnel ingress + Cloudflare Access (replaces nginx) |

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
