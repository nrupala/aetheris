# Aetheris Development Guide

## Project Overview

**Aetheris** is a Sovereign AI-Native Personal Cloud built with Rust, designed for secure, self-hosted deployment. It combines zero-trust security principles with AI-powered policy enforcement.

## Quick Start

```bash
# Build and run
cargo build --release
./target/release/aetheris

# Run tests
cargo test --all

# Docker build
docker compose build
docker compose up -d
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

## Project Structure

```
aetheris/
├── core/                    # Rust source code
│   ├── src/
│   │   ├── main.rs         # Entry point, Axum server
│   │   ├── lib.rs          # Module exports
│   │   ├── sync.rs         # Upload/download handlers
│   │   ├── connector.rs    # AI/OPA integration
│   │   └── watcher.rs      # Security watcher
│   └── Cargo.toml
├── compose.yaml             # Docker Compose stack
├── Dockerfile.core          # Rust container
├── scripts/                 # Deployment scripts
│   ├── bootstrap.sh         # System init
│   ├── verification.sh      # Integrity checks
│   └── killswitch.sh       # Emergency purge
└── .github/workflows/       # CI/CD pipelines
```

## Key Technologies

- **Web Framework**: Axum (async Rust)
- **Policy Engine**: OPA (Open Policy Agent)
- **Storage**: ZFS with native encryption
- **VPN**: WireGuard for mesh networking
- **Container Runtime**: Docker Compose
- **CI/CD**: GitHub Actions

## Development Workflow

### Making Changes

1. Create a feature branch: `git checkout -b feature/my-feature`
2. Make changes and test locally
3. Run full test suite: `cargo test --all`
4. Commit with clear message
5. Push and create PR

### Testing

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test '*'

# UAT tests (requires Docker)
docker compose up -d
./scripts/verification.sh
```

## Coding Standards

- Run `cargo fmt` before committing
- Run `cargo clippy -- -D warnings` for linting
- All public APIs must have documentation comments
- Error handling with Result types, no panics in library code

## Common Tasks

### Adding a New API Endpoint

1. Add handler in `core/src/sync.rs`
2. Register route in `core/src/main.rs`
3. Add tests in `core/tests/`
4. Update UAT tests if behavior changes

### Modifying Policy Engine

1. Update OPA policies in `core/policies/`
2. Test locally with `opa eval`
3. Update connector.rs if API changes

### Docker Changes

1. Test locally: `docker compose build && docker compose up -d`
2. Verify logs: `docker compose logs -f`
3. Check health: `curl http://localhost:8080/health`

## Troubleshooting

### Build Failures

```bash
# Clean and rebuild
cargo clean
cargo build --release
```

### Docker Issues

```bash
# Reset containers
docker compose down -v
docker compose build --no-cache
docker compose up -d
```

### Test Failures

```bash
# Run with verbose output
cargo test -- --nocapture
RUST_LOG=debug cargo test
```

## CI/CD Pipeline

- **build.yml**: Runs on every PR/push to main
- **uat.yml**: Runs UAT tests on merge to main
- **pages.yml**: Deploys documentation to GitHub Pages
- **deploy.yml**: Pushes images to GHCR on release

## Getting Help

- Check `TODO.md` for current status
- Review `UAT_RESULTS.md` for test coverage
- See `README.md` for project overview
