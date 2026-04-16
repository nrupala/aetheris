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

## Containerized File Management
Aetheris provides a secure, containerized file management system that leverages ZFS technology with native encryption. Key features include:

1. **Secure Storage**: Files are stored in encrypted containers using ZFS's built-in encryption capabilities.

2. **Access Control**: Fine-grained access control through the AI-powered policy engine (OPA) ensures only authorized users can access specific files or directories.

3. **Versioning & Snapshots**: ZFS provides built-in snapshot capabilities for easy recovery of previous versions of files.

4. **Cross-Platform Compatibility**: The containerized approach ensures consistent behavior across different environments while maintaining security standards.

5. **Scalability**: The architecture supports horizontal scaling through container orchestration, allowing the system to grow with user needs.

6. **Audit Trail**: All file operations are logged and can be audited for compliance purposes.

7. **Backup & Recovery**: Built-in backup mechanisms ensure data integrity and provide recovery options in case of failures.

8. **Performance Optimization**: The containerized approach allows for optimized resource allocation, ensuring efficient use of system resources while maintaining performance.

9. **Zero-Trust Security**: Files are never stored in plaintext; all operations occur within encrypted containers with strict access controls.

10. **Containerization**: All file operations occur within isolated containers, providing additional security boundaries and preventing unauthorized access to system resources.

## Core Features

### Zero-Trust Security
- **OPA Policy Evaluation**: Every request is evaluated by OPA policies for fine-grained access control.
- **Dynamic Authorization**: Real-time policy enforcement based on context and identity.
- **Audit Logging**: Comprehensive logging of all security-related events.

**Skills for Zero-Trust Security:**
- policy-engine: Work with OPA (Open Policy Agent) Rego policies, policy evaluation, and authorization
- identity-management: OpenID Connect, OAuth 2.0, JWT validation, user authentication
- audit-logging: Log analysis, SIEM integration, compliance auditing, security event tracking
- access-control: RBAC, ABAC, least privilege, permission modeling

### WireGuard Mesh
- **L3 Encrypted Tunneling**: Invisible encrypted tunnel using UDP 51820 only.
- **Mesh Networking**: Self-healing network topology with automatic re-routing.
- **Secure Communication**: End-to-end encryption for all inter-node communications.

**Skills for WireGuard Mesh:**
- wireguard: WireGuard VPN configuration, key management, peer configuration, mesh networking
- network-security: VPN setup, firewall rules, network segmentation, encrypted tunneling
- udp-networking: UDP protocols, port 51820, NAT traversal, hole punching
- mesh-networking: Self-healing networks, automatic failover, distributed systems

### Local AI
- **Ollama Integration**: On-device semantic search capabilities powered by Ollama.
- **Contextual Search**: Advanced search functionality based on file content and metadata.
- **AI-Powered Indexing**: Intelligent indexing of files with semantic understanding.

**Skills for Local AI:**
- ollama: Ollama API integration, local LLM deployment, model management
- semantic-search: Vector embeddings, similarity search, RAG implementation
- ai-indexing: Content extraction, embedding generation, metadata tagging
- rust-ai: Integrating AI libraries in Rust, async AI processing

### ZFS Encryption
- **AES-256-GCM at Rest**: End-to-end encryption using industry-standard AES-256-GCM algorithm.
- **Full Disk Encryption**: Complete protection of all stored data.
- **Key Management**: Secure key management and rotation mechanisms.

**Skills for ZFS Encryption:**
- zfs: ZFS storage pools, datasets, snapshots, clones, compression
- encryption: AES-256-GCM, key derivation, secure key storage, key rotation
- disk-encryption: dm-crypt, LUKS, full disk encryption, key escrow
- secure-storage: Encrypted volumes, key management systems, HSM integration

### Zero-JS UI
- **Server-Side Rendering**: HTML rendered on the server for enhanced security.
- **No Client-Side JavaScript**: Reduced attack surface by eliminating client-side execution.
- **Content Security Policy (CSP)**: Enhanced security through CSP implementation.

**Skills for Zero-JS UI:**
- server-side-rendering: SSR, HTML templating, server-rendered pages
- web-security: CSP headers, XSS prevention, CSRF protection
- axum: Rust Axum web framework, routing, middleware, request handling
- html-css: Semantic HTML, accessibility, responsive design without JS

### Ghost Shell
- **High-Interaction Honeypot**: Advanced honeypot technology to detect and analyze threats.
- **Isolated Environment**: Operations occur in isolated containers with minimal privileges.
- **Behavioral Analysis**: Monitoring of suspicious activities for threat detection.

**Skills for Ghost Shell:**
- honeypot: High-interaction honeypots, deception technology, threat detection
- container-security: Container isolation, minimal privilege containers, seccomp
- threat-detection: Behavioral analysis, anomaly detection, intrusion detection
- forensic-analysis: Log analysis, attack reconstruction, incident response

### Kill-Switch
- **Emergency Protocol**: Comprehensive scorched earth protocol for critical situations.
- **Data Protection**: Automatic encryption and deletion of sensitive data.
- **System Isolation**: Complete isolation from network connections during emergency mode.

**Skills for Kill-Switch:**
- emergency-response: Incident response, emergency protocols, crisis management
- secure-deletion: Data destruction, secure wipe, forensic deletion
- network-isolation: Firewalls, network cut-off, isolation mechanisms
- disaster-recovery: Backup restoration, failover systems, business continuity