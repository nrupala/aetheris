# Aetheris - Sovereign AI-Native Personal Cloud

![Build Status](https://github.com/nrupala/aetheris/actions/workflows/build.yml/badge.svg)
![Pages](https://github.com/nrupala/aetheris/actions/workflows/pages.yml/badge.svg)

**Aetheris** is a zero-trust, zero-knowledge personal cloud system built for the agentic age. It replaces commercial edge providers like Cloudflare with an invisible, encrypted FOSS mesh.

## Features

- **Zero-Trust Security**: Every request evaluated by OPA policy
- **WireGuard Mesh**: Invisible L3 encrypted tunnel (UDP 51820 only)
- **Local AI**: Ollama-powered semantic search
- **ZFS Encryption**: AES-256-GCM at rest
- **Zero-JS UI**: Server-side rendered HTML
- **Ghost Shell**: High-interaction honeypot
- **Kill-Switch**: Emergency Scorched Earth Protocol

## Quick Start

### Prerequisites
- Ubuntu 22.04 LTS / Debian 12
- Docker/Podman
- ZFS utilities
- 8GB RAM, 500GB storage

### Install
```bash
git clone https://github.com/nrupala/aetheris.git
cd aetheris

chmod +x scripts/bootstrap.sh
sudo ./scripts/bootstrap.sh

./scripts/verification.sh
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    WIREGUARD MESH                            │
│               (10.0.0.0/24 - Zero-Trust)                  │
└─────────────────────────────────────────────────────────────┘
                          │
    ┌─────────────────────┼─────────────────────┐
    ▼                     ▼                     ▼
┌──────────────┐   ┌──────────────┐   ┌──────────────┐
│   CORE       │   │   AI ENGINE  │   │  VECTOR DB   │
│   (Rust)     │◄──│  (Ollama)    │◄──│  (ChromaDB)  │
│   :8080      │   │   :11434     │   │   :8000      │
└──────────────┘   └──────────────┘   └──────────────┘
```

## Documentation

- [Architecture](ARCHITECTURE.md)
- [API Specification](API_SPEC.md)
- [Test Plan](TEST_PLAN.md)
- [UAT Tests](tests/UAT_TESTS.md)
- [Build Plan](BUILD_PLAN.md)
- [Requirements](REQUIREMENTS.md)

## Security

Aetheris is designed with security first:

1. **No Public Ports**: Only UDP 51820 (WireGuard) exposed
2. **Zero-Trust**: OPA policy denies by default
3. **E2EE**: Keys never leave your hardware
4. **Ghost Shell**: Honeypot traps attackers
5. **Auto-Ban**: 5 failures = 1 hour ban
6. **Kill-Switch**: Instant vault lockdown

## License

MIT License - See [LICENSE](LICENSE) for details.

---

**Version:** 1.0.0  
**Status:** Production Ready  
**Date:** 2026-04-15
