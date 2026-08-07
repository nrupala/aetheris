# Aetheris Development Guide

## Project Overview
**Aetheris** is a Sovereign AI-Native Personal Cloud built with Rust, designed for secure, self-hosted deployment. It combines zero-trust security principles with AI-powered policy enforcement.

## Quick Start
```bash
# Build and run
cargo build --release
./target/release/aetheris-core

# Run tests
cargo test --all

# Native deploy (systemd, no Docker)
sudo scripts/install-native.sh
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
│   │   ├── main.rs         # Entry point, Axum server (847 lines, 38+ endpoints)
│   │   ├── lib.rs          # Module exports
│   │   ├── bridge.rs       # Trait abstractions (AetherisBridge, SecurityBridge, ModelBridge)
│   │   ├── implementation.rs  # OllamaBridge + OpaBridge implementations
│   │   ├── sync.rs         # Upload/download handlers (sync_router)
│   │   ├── proxy.rs        # Reverse proxy to Python orchestrator
│   │   ├── wal.rs          # Write-ahead log with 9 entry types
│   │   ├── a2a.rs          # Agent-to-Agent messaging gateway
│   │   ├── mcp.rs          # MCP protocol with 9 tool definitions
│   │   └── agents/         # Agent trait, BaseAgent, factory, 4 implementations
│   │       ├── mod.rs      # Agent/AgentRole/AgentState/AgentResult/BaseAgent
│   │       ├── planner.rs  # Task decomposition agent
│   │       ├── researcher.rs  # RAG query agent
│   │       ├── coder.rs    # Code generation agent
│   │       └── reviewer.rs # Quality review agent
│   └── Cargo.toml
├── web/                    # HTML subdomain UIs
│   ├── ai/index.html       # AI Chat with help panel
│   ├── agents/index.html   # Agent Dashboard with helps panel
│   ├── rag/index.html      # RAG Document Q&A with help panel
│   └── dev/index.html      # Dev Sandbox with help panel
├── infra/systemd/          # systemd units (native deploy)
├── config/core.env.example # Non-secret core environment template
├── scripts/install-native.sh  # Native (no-Docker) installer
├── docs/                    # Restructured documentation (AppDocs format)
│   ├── architecture/        # System architecture docs
│   ├── api-reference/       # API specs
│   ├── guides/             # User/developer/ops guides
│   ├── WHAT_NOT_TO_DO.md   # Anti-patterns
│   ├── COMMANDS.md         # Command reference
│   ├── INTERFACES.md       # Interface definitions
│   ├── ROLE_GUIDES.md      # Role-based per-persona docs
│   └── FAQ.md              # Frequently asked questions
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
- **Runtime**: Native systemd (no Docker)
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

# UAT tests (native)
sudo scripts/install-native.sh
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

### Deploy Changes
1. Test locally: `cargo build --release && ./target/release/aetheris-core`
2. Verify logs: `journalctl -u aetheris-core -f`
3. Check health: `curl http://localhost:8080/health`

## Troubleshooting
### Build Failures
```bash
# Clean and rebuild
cargo clean
cargo build --release
```

### Service Issues
```bash
# Reset the native service
systemctl restart aetheris-core
sudo scripts/install-native.sh
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
- **release.yml**: Cross-compiles release binaries on tag

## Getting Help
- Check `TODO.md` for current status
- See `README.md` for project overview
- Review `docs/COMMANDS.md` for common command reference
- Review `docs/FAQ.md` for common questions
- Review `docs/WHAT_NOT_TO_DO.md` for anti-patterns

## Build Status

### Security posture — OPA authorization (LIVE)
- **`OPA_ENFORCE=1` in `/etc/aetheris/core.env`** — enforcement **LIVE** since `2026-08-04 02:48 UTC` (verified: core.env mtime = last core restart). A single flag governs **both** the HTTP `opa_gate` middleware **and** agent `check_policy`.
- **Post-flip soak (24h+, to `2026-08-05T08:19Z`):** `2163` OPA decisions, **`0` would-deny**, **`0` admin-identity denies**, **`0` agent would-denies**. Panel/admin browsing, health GETs, and agent runs **all allow**; the middleware is confirmed firing on every route.
- **Rollback:** set `OPA_ENFORCE=0` in `/etc/aetheris/core.env` + `systemctl restart aetheris-core` (instant, no rebuild).
- **Scope:** gates the method-aware `is_sensitive` set (mutating verbs + secret/log/file reads); non-sensitive GETs stay open. Core binds `127.0.0.1:8080`; only the cloudflared tunnel reaches it.
- **Identity today** = plaintext `Cf-Access-Authenticated-User-Email` header (safe only via loopback+iptables). Hardening in `docs/OPA_P5_JWT_PLAN.md`. **P5 JWT verification LIVE** (`CF_JWT_VERIFY=1`, since `2026-08-06`): `verify_assertion` (`src/auth/cf_jwt.rs`, RS256 via `jsonwebtoken`+`rust_crypto`, pinned JWKS) on `is_sensitive` routes — **the verified JWT email is the authoritative identity**; an unverifiable/missing/forged `Cf-Access-Jwt-Assertion` degrades to `unknown` (denied on sensitive routes), closing the plaintext-header spoof gap. Shadow mode was soak-clean (48h, only synthetic observations). **Rollback = `CF_JWT_VERIFY=0` + `systemctl restart aetheris-core`** (no rebuild; independent of `OPA_ENFORCE`). JWKS refreshed hourly (`cf-access-jwks.timer`, keep last-good).
- **Known-expected 403:** `infra/tests/smoke_health.sh` `POST /bridge/ai/embed` (header-less write). Mint a scoped CF-Access service token only if write-route smoke tests must run.
- **DO NOT silently flip `OPA_ENFORCE` off** — intentional live control. Only the operator (Milo) authorizes a change.

### Completed
- **Core compilation** ✅ — `cargo check` passes with zero errors
- **`workflow_run_handler` fix** ✅ — `std::sync::MutexGuard` was held across `.await`, making future non-`Send`. Fixed by extracting agent from lock, dropping guard, awaiting, then re-acquiring.
- **Agent pipeline** ✅ — Planner/Researcher/Coder/Reviewer with A2A messaging, MCP tools
- **WAL** ✅ — 9 entry types, append/replay/truncate, 11 tests
- **Bridge traits** ✅ — `ModelBridge`, `SecurityBridge`, `AetherisBridge` with `Send + Sync`
- **Native systemd deploy** ✅ — install-native.sh + aetheris-core.service (no Docker)
- **Documentation** ✅ — Restructured to AppDocs format with role-based guides
- **Help panels** ✅ — All 4 subdomain UIs have comprehensive help panels
- **Reranker** ✅ — `rerank()` added to `ModelBridge` trait, implemented via Ollama `/api/rerank`, wired into `rag_query_handler` with `RagConfig.reranker_model`/`reranker_enabled`. ⚠️ **Disabled by default** — deployed Ollama 0.24.0 has no `/api/rerank` (404); queries fall back to vector-search order and are always truncated to `top_k`.
- **Model set 2026** ✅ — Updated defaults: `qwen3:8b` (primary / deep reasoning), `bge-reranker-v2-m3` (reranker, not installed on box), `phi4-mini` (lightweight — **RAG default**), `phi4-reasoning` (full reasoning)
- **RAG pipeline live** ✅ — End-to-end RAG on `nomic-embed-text` (embed) + `phi4-mini` (generate) on CPU: embed → search → generate, sources indexed in SQLite `vectors.db`, working via localhost:8080 and cloudflared at rag.nrupalakolkar.com with `top_k=5`, 300s timeout, `took_ms` timing fixed
- **RAG config save** ✅ — `PUT /config` accepts partial payloads (merged over current config) and persists `rag_config.json`; `serde(default)` + merge semantics
- **RAG production-readiness pass** ✅ — config-save 422 fixed (partial PUT merge), rerank-failure no longer bloats context (always truncates to `top_k`), `/sources` & `/stats` exclude sqlite/WAL artifacts, `DELETE /sources/{name}` purges vector chunks (FK cascade), `/health` reports real system memory, embed-model dimension guard prevents silent index corruption
- **OPA Phase 1 — authz contract** ✅ — `AuthzInput {identity,role,method,path,action}` contract, `OpaBridge.authorize` POSTs `/v1/data/aetheris/authz/allow`, parses `result.as_bool().unwrap_or(false)`, fail-open (`OPA_FAIL_OPEN`, default true) + bump `aetheris_security_violations_total`; `config/policy/aetheris.authz.rego` (OPA v1 `if`, versioned, no longer generated by bootstrap); `identity_to_role` pure helper (not wired). Enforcement off (`opa_enforce` default false).
- **OPA Phase 2 — native opa.service on box** ✅ — `install-native.sh` installs static OPA `v1.1.0` arm64 → `/usr/local/bin/opa`, deploys `aetheris.authz.rego` → `/etc/aetheris/policy`, enables `infra/systemd/opa.service` (loopback `127.0.0.1:8181`, `decision_logs.console=true`); `OPA_ENDPOINT=http://127.0.0.1:8181`; `bridge_ai_query` gate now respects `opa_enforce` (logs + bumps violations, only 403s when enforcing) so standing up OPA does not break `/bridge/ai/query`. Docker-OPA remnants stripped from `REQUIREMENTS.md`/`docs/guides/deployment.md`.
- **OPA Phase 3 — shadow middleware (blocks nothing)** ✅ — `opa_gate` (`axum::middleware::from_fn_with_state`) applied to the whole router right before `.with_state(state)`; evaluates every request, logs + bumps `aetheris_security_violations_total` on would-deny, and only 403s when `opa_enforce && is_sensitive(...)` (off this phase → shadow only). `access_role(email, Cf-Access-Client-Id)` → `identity_to_role` (admin / analyst / unknown). 6 tests (shadow pass-through, would-deny logs+bumps+passes, allow pass, enforce-blocks-sensitive, is_sensitive, identity map).
- **OPA Phase 4 PREP (enforce still off)** ✅ — (a) `is_sensitive` re-scoped to be **method-aware** (`is_mutating`: POST/PUT/PATCH/DELETE, or GET of `/keys,/audit,/sync/download,/dev/logs`) — drops the blanket `/bridge,/upload,/ingest,/task,/workflow,/sync/upload` prefix (those are write-only, caught by verb); clears every non-mutating header-less GET from the soak. (b) Core bound to **`127.0.0.1:8080`** (loopback-only; tunnel is sole ingress) — internal-caller trust is now a service-token header, not source-IP. (c) Removed dead nginx/`dev_user` htpasswd cred from `verification.sh` (basic-auth retired under CF Access). Soak decision-log confirms: **zero denies for Milo's admin identity**; the mutating header-less callers (`POST /ingest/file`, `POST /bridge/ai/query`) are OpenCode verification one-offs (→ 403 on flip, no service token); agent `check_policy` **hard-blocks** (`failed/Policy denied`) → agents degraded in shadow, need `data.aetheris.agents` decouple before flip.
- **OPA agent-authz decouple** ✅ — new `config/policy/aetheris.agents.rego` (separate `package aetheris.agents`, default deny, per-role action allowlists enumerated from the `check_policy` local fallback: researcher/coder/reviewer/planner). `SecurityBridge.authorize_agent` POSTs `/v1/data/aetheris/agents/allow` (not `/authz/allow`) via a shared `OpaBridge::eval(decision_path, input)`; `OpaBridge` now carries `enforce`/`fail_open`. **`check_policy` is opa_enforce-aware**: shadow (enforce off) → `log::warn!` + bump + **advisory allow** (restores agents); enforce → honor the deny. **Model invocation is in-process**: agents use `call_llm` → `bridge.query` (OllamaBridge, direct) — **not** a self-HTTP to `/bridge/ai/query`, so the middleware does not gate agent LLM and no service token is needed. Tests: each role's allowed actions pass / unknown role-action denies / agent-path routing, advisory-allows on would-deny / enforce blocks / no-bridge local allowlist; metrics tests hardened (gauge folded, counter non-decreasing) for parallel-harness races. **Analyst not in `aetheris.agents`**: `analyst` is only the HTTP service-token identity role (`identity_to_role`), never an AgentRole — dropped from the agent package before enforce.

### OPA Phase 3 (BUILT) — seam reference
One-line middleware add in the `main()` app builder, immediately before `.with_state(state)` (currently the `.route("/v1/*path", …)` line):

```rust
.route("/v1/*path", get(v1_proxy_handler).post(v1_proxy_handler))
.layer(axum::middleware::from_fn_with_state(state.clone(), opa_gate)) // ADD
.with_state(state);
```

```rust
async fn opa_gate(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let email = req.headers().get("Cf-Access-Authenticated-User-Email")
        .and_then(|v| v.to_str().ok()).unwrap_or("");
    let role = identity_to_role(email); // + Cf-Access-Client-Id -> analyst
    let input = bridge::AuthzInput { identity: email.into(), role: role.into(),
        method: req.method().as_str().into(), path: req.uri().path().into(), action: "http".into() };
    let allowed = state.security_bridge.authorize(&input).await;
    if !allowed {
        log::warn!("OPA would DENY {} {} ({})", input.method, input.path, if email.is_empty(){"<none>"}else{email});
        metrics::SECURITY_VIOLATIONS.inc();
        if state.opa_enforce && is_sensitive(&input.method, &input.path) { // D2 scope
            return (StatusCode::FORBIDDEN, Json(json!({"error":"Policy denied"}))).into_response();
        }
    }
    next.run(req).await
}
```
Prereqs in place (all built): `AppState.opa_enforce` (P2), `identity_to_role`/`AuthzInput` (P1), `is_sensitive(method, path)` + `access_role` + `opa_gate` (P3). **Enforcement is LIVE** (`OPA_ENFORCE=1`, see "Security posture — OPA authorization (LIVE)" above); rollback = `OPA_ENFORCE=0` + restart.

### In Progress
- **WAL-backed dev logs** — logs endpoint initialized but not dynamically appended
- **Agent state `Waiting`** — defined in enum but never reached in sequential execution
- **UAT tests** — need a native deployment to run verification scripts

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
- **Reranker Pipeline**: Cross-encoder reranking via `bge-reranker-v2-m3` for second-pass relevance scoring after vector search.

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