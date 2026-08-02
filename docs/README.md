# Aetheris Documentation

**Aetheris** is a Sovereign AI-Native Personal Cloud built with Rust — designed for secure, self-hosted deployment with zero-trust security and AI-powered policy enforcement.

## Quick Start

```bash
# Build and run Rust core
cd core && cargo build --release
./target/release/aetheris

# Native install (systemd, no Docker)
sudo scripts/install-native.sh
```

## Documentation Map

### Architecture
| Document | Description |
|----------|-------------|
| [Architecture Overview](architecture/overview.md) | System design, network topology, component architecture, data flows |
| [Emulation Environment](architecture/emulation.md) | Docker-based bare metal emulation for testing |
| [Port Allocation](architecture/port-allocation.md) | Dynamic port allocation and service discovery |

### API Reference
| Document | Description |
|----------|-------------|
| [API Specification](api-reference/README.md) | Complete REST API with request/response schemas |

### Guides
| Document | Description |
|----------|-------------|
| [Getting Started](guides/getting-started.md) | Installation and first-run guide |
| [User Guide](guides/user-guide.md) | End-user guide for RAG and document Q&A |
| [Application & Usage Guide](guides/application-guide.md) | 10 use cases with deployment options and extensions |
| [Deployment Guide](guides/deployment.md) | Production, development, and local deployment |
| [Security Guide](guides/security.md) | Security principles, WireGuard, encryption, OPA policies |
| [Agent Orchestrator](guides/agent-orchestrator.md) | Multi-agent coordination with MCP, A2A, workflow engine |
| [RAG Pipeline](guides/rag-pipeline.md) | Retrieval-Augmented Generation from upload to answer |
| [Knowledge Graph](guides/knowledge-graph.md) | Personal context layer and memory system |
| [Coordinator](guides/coordinator.md) | Central governance layer with circuit breakers and resource monitoring |
| [Observability](guides/observability.md) | APM monitoring, performance analysis, anomaly detection |
| [Self-Evaluator](guides/self-evaluator.md) | Continuous improvement system with scoring dimensions |
| [Test Plan](guides/test-plan.md) | Production validation with 27 tests across 6 categories |

### Reference
| Document | Description |
|----------|-------------|
| [Common Commands](COMMANDS.md) | Frequently used CLI commands and shortcuts |
| [Interface Definitions](INTERFACES.md) | Core traits, types, and protocol definitions |
| [FAQ](FAQ.md) | Frequently asked questions |
| [What Not To Do](WHAT_NOT_TO_DO.md) | Anti-patterns and common mistakes to avoid |
| [Role-Based Guides](ROLE_GUIDES.md) | Per-persona guides (User, Developer, Maintainer, Admin, Ops, Regulator) |

### Roadmap
| Document | Description |
|----------|-------------|
| [Phase 2 Roadmap](PHASE2_ROADMAP.md) | Architecture roadmap for isolation, encryption, efficiency, and autonomy |

## Project Status

See `AGENTS.md` in the project root for the current build status and development progress.

## License

Aetheris is licensed under the MIT License. See `LICENSE` in the project root.
