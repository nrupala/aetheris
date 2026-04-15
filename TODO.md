# AETHERIS - BUILD TODO LIST
## Production Release Tracking

**Start Date:** 2026-04-15
**Target Duration:** 14 days
**Status:** IN PROGRESS

---

## PHASE 1: FILE EXTRACTION

| # | Task | Status | Dependencies |
|---|------|--------|--------------|
| 1.1 | Create directory structure | ⬜ | None |
| 1.2 | Extract Rust source files (Cargo.toml, main.rs, sync.rs, etc.) | ⬜ | 1.1 |
| 1.3 | Extract Docker configuration (compose.yaml, Dockerfile) | ⬜ | 1.1 |
| 1.4 | Extract OPA policy files (policy.rego, opa_conf.yaml) | ⃝ | 1.1 |
| 1.5 | Extract bash scripts (bootstrap.sh, verification.sh, etc.) | ⬜ | 1.1 |
| 1.6 | Extract HTML UI (index.html) | ⬜ | 1.1 |
| 1.7 | Extract config files (wg0.conf, state.json, config.yaml) | ⬜ | 1.1 |
| 1.8 | Extract monitoring config (zrepl.yml, sentinel_dashboard.json) | ⬜ | 1.1 |
| 1.9 | Extract sentinel agent (ai_sentinel.py) | ⬜ | 1.1 |
| 1.10 | Validate all extracted files | ⬜ | 1.2-1.9 |

---

## PHASE 2: RUST CORE BUILD

| # | Task | Status | Dependencies |
|---|------|--------|--------------|
| 2.1 | Install Rust toolchain | ⬜ | None |
| 2.2 | Add MUSL target | ⬜ | 2.1 |
| 2.3 | Create Cargo.toml with dependencies | ⬜ | 1.2 |
| 2.4 | Implement main.rs (entry point + watcher + guard) | ⬜ | 1.2 |
| 2.5 | Implement sync.rs (upload/download handlers) | ⬜ | 1.2 |
| 2.6 | Implement connector.rs (AI/OPA bridge) | ⬜ | 1.2 |
| 2.7 | Implement bridge.rs + implementation.rs | ⬜ | 1.2 |
| 2.8 | Implement metrics.rs (Prometheus) | ⬜ | 1.2 |
| 2.9 | Implement watcher.rs (security auto-ban) | ⬜ | 1.2 |
| 2.10 | Build Rust binary (musl static) | ⬜ | 2.4-2.9 |
| 2.11 | Verify binary size and functionality | ⬜ | 2.10 |
| 2.12 | Create multi-stage Dockerfile | ⬜ | 2.10, 1.3 |
| 2.13 | Build Distroless container image | ⬜ | 2.12 |

---

## PHASE 3: CONTAINER STACK

| # | Task | Status | Dependencies |
|---|------|--------|--------------|
| 3.1 | Create compose.yaml | ⬜ | 1.3 |
| 3.2 | Create docker-compose.ghost.yaml | ⬜ | 1.3 |
| 3.3 | Configure WireGuard (wg0.conf) | ⬜ | 1.7 |
| 3.4 | Configure OPA (policy.rego, opa_conf.yaml) | ⬜ | 1.4 |
| 3.5 | Configure zrepl (zrepl.yml) | ⬜ | 1.8 |
| 3.6 | Build all container images | ⬜ | 3.1-3.5 |
| 3.7 | Pull Ollama AI models | ⬜ | 3.6 |
| 3.8 | Verify all containers start | ⬜ | 3.6 |
| 3.9 | Test container networking | ⬜ | 3.8 |

---

## PHASE 4: HOST PREPARATION (Bare Metal)

| # | Task | Status | Dependencies |
|---|------|--------|--------------|
| 4.1 | Install ZFS | ⬜ | None |
| 4.2 | Create encrypted ZFS pool | ⬜ | 4.1 |
| 4.3 | Run vault_setup.sh | ⬜ | 4.2, 1.5 |
| 4.4 | Install Docker/Podman | ⬜ | None |
| 4.5 | Install WireGuard | ⬜ | None |
| 4.6 | Generate WireGuard keys | ⬜ | 4.5 |
| 4.7 | Configure systemd services | ⬜ | 4.4-4.6 |
| 4.8 | Start WireGuard mesh | ⬜ | 4.6, 4.7 |

---

## PHASE 5: INTEGRATION TESTS

| # | Task | Status | Dependencies |
|---|------|--------|--------------|
| 5.1 | Run verification.sh | ⬜ | 3.8, 4.8 |
| 5.2 | Test Core API (8 tests) | ⬜ | 5.1 |
| 5.3 | Test Security API (6 tests) | ⬜ | 5.1 |
| 5.4 | Test AI API (5 tests) | ⬜ | 5.1 |
| 5.5 | Test Storage (4 tests) | ⬜ | 5.1 |
| 5.6 | Test Monitoring (4 tests) | ⬜ | 5.1 |
| 5.7 | All 27 tests passing | ⬜ | 5.2-5.6 |

---

## PHASE 6: CLIENT DEPLOYMENT

| # | Task | Status | Dependencies |
|---|------|--------|--------------|
| 6.1 | Generate Android client config | ⬜ | 4.6 |
| 6.2 | Generate iOS client config | ⬜ | 4.6 |
| 6.3 | Generate Windows client config | ⬜ | 4.6 |
| 6.4 | Generate macOS client config | ⬜ | 4.6 |
| 6.5 | Generate Linux client config | ⬜ | 4.6 |
| 6.6 | Test peer connection (all platforms) | ⬜ | 6.1-6.5 |
| 6.7 | Test file sync between peers | ⬜ | 6.6 |

---

## PHASE 7: MONITORING & SECURITY

| # | Task | Status | Dependencies |
|---|------|--------|--------------|
| 7.1 | Configure VictoriaMetrics | ⬜ | 3.8 |
| 7.2 | Import Grafana dashboard | ⬜ | 7.1 |
| 7.3 | Configure AI Sentinel | ⬜ | 7.1, 1.9 |
| 7.4 | Test auto-ban (5 failures) | ⬜ | 5.3 |
| 7.5 | Test Ghost Shell redirect | ⬜ | 5.3 |
| 7.6 | Configure alert notifications | ⬜ | 7.2 |

---

## PHASE 8: BACKUP & RECOVERY

| # | Task | Status | Dependencies |
|---|------|--------|--------------|
| 8.1 | Configure offsite replication | ⬜ | 4.3 |
| 8.2 | Test offsite backup | ⬜ | 8.1 |
| 8.3 | Configure USB cold storage | ⬜ | 4.3 |
| 8.4 | Test vault_rollback.sh | ⬜ | 4.3, 1.5 |
| 8.5 | Test ERM-01 recovery | ⬜ | 8.1 |
| 8.6 | Verify kill-switch (dry-run) | ⬜ | 4.3, 1.5 |

---

## PHASE 9: PRODUCTION HARDENING

| # | Task | Status | Dependencies |
|---|------|--------|--------------|
| 9.1 | Remove default passwords | ⬜ | All |
| 9.2 | Configure firewall rules | ⬜ | 4.5 |
| 9.3 | Enable audit logging | ⬜ | 7.1 |
| 9.4 | Backup master key (offsite) | ⬜ | 4.3 |
| 9.5 | Document recovery procedures | ⬜ | 8.5 |
| 9.6 | Security audit | ⬜ | 9.1-9.5 |
| 9.7 | **PRODUCTION READY** | ⬜ | 5.7, 6.6, 9.6 |

---

## PROGRESS SUMMARY

| Phase | Total | Completed | In Progress | Pending |
|-------|-------|-----------|-------------|---------|
| Phase 1 | 10 | 0 | 0 | 10 |
| Phase 2 | 13 | 0 | 0 | 13 |
| Phase 3 | 9 | 0 | 0 | 9 |
| Phase 4 | 8 | 0 | 0 | 8 |
| Phase 5 | 7 | 0 | 0 | 7 |
| Phase 6 | 7 | 0 | 0 | 7 |
| Phase 7 | 6 | 0 | 0 | 6 |
| Phase 8 | 6 | 0 | 0 | 6 |
| Phase 9 | 7 | 0 | 0 | 7 |
| **TOTAL** | **73** | **0** | **0** | **73** |

---

## LEGEND

- ⬜ Not Started
- 🟡 In Progress
- ⃝ Blocked (waiting on dependency)
- ✅ Complete

---

**LAST UPDATED:** 2026-04-15
**UPDATED BY:** Autonomous Build Agent
