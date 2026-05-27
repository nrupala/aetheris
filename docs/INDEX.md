# Aetheris - Sovereign AI-Native Personal Cloud

![Build Status](https://github.com/nrupala/aetheris/actions/workflows/build.yml/badge.svg)
![UAT Tests](https://github.com/nrupala/aetheris/actions/workflows/uat.yml/badge.svg)
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

### Getting Started
- [**User Guide**](docs/USER_GUIDE.md) — How to upload documents, ask questions, manage your knowledge base
- [**Application & Usage Guide**](docs/APPLICATION_AND_USAGE_GUIDE.md) — Use cases, deployment scenarios, workflows, integration patterns
- [**Documentation Index**](docs/INDEX.md) — Complete documentation map and reading order

### Architecture & Design
- [**RAG Pipeline**](docs/RAG_PIPELINE.md) — Complete workflow from upload to answer, with diagrams
- [**Processing Coordinator**](docs/COORDINATOR.md) — State machine, circuit breaker, error handling, audit logging
- [**Knowledge Graph**](docs/KNOWLEDGE_GRAPH.md) — Personal context layer, entity-relation model, query enrichment
- [**Observability**](docs/OBSERVABILITY.md) — Performance monitoring, anomaly detection, system event logging
- [**Self-Evaluator**](docs/SELF_EVALUATOR.md) — Continuous improvement system, session analysis, auto-suggestions
- [**Port Allocation**](docs/PORT_ALLOCATOR.md) — Dynamic port assignment and service discovery
- [**Extending Aetheris**](docs/APPLICATION_AND_USAGE_GUIDE.md#9-extending-aetheris) — Adding models, agents, MCP tools, resources, prompts

### Project Management
- [**Architecture**](ARCHITECTURE.md) — System architecture
- [**API Specification**](API_SPEC.md) — REST API reference
- [**Build Plan**](BUILD_PLAN.md) — Phase roadmap, gap analysis, task tracking
- [**Test Plan**](TEST_PLAN.md) — Testing strategy
- [**Requirements**](REQUIREMENTS.md) — Functional requirements
- [**Security**](SECURITY.md) — Security considerations

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
**Status:** CI/CD Passing | UAT Passing | Ready for Deployment  
**Date:** 2026-04-15  
**Progress:** 43% (Phase 1-2 Complete, Phase 3 In Progress)
