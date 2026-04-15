# AETHERIS - AUTONOMOUS BUILD PERMISSIONS
## 4-Hour Autonomous Operation Authorization

**Date:** 2026-04-15
**Duration:** 4 hours
**Agent Mode:** BUILD (read-write-execute)

---

## AUTHORIZED OPERATIONS

### 1. FILE OPERATIONS ✓
| Operation | Permission | Scope |
|-----------|------------|-------|
| Create directories | GRANTED | /opt/aetheris/* |
| Create files | GRANTED | All embedded artifacts |
| Read existing files | GRANTED | Aetheris codebase |
| Write/Edit files | GRANTED | Build artifacts |
| Delete temp files | GRANTED | Build process only |

### 2. SHELL COMMANDS ✓
| Command Type | Permission | Notes |
|--------------|------------|-------|
| apt/dpkg install | GRANTED | System dependencies |
| cargo build | GRANTED | Rust compilation |
| docker build | GRANTED | Container builds |
| docker compose | GRANTED | Stack management |
| docker exec | GRANTED | Container interaction |
| curl/wget | GRANTED | API testing |
| git clone/pull | GRANTED | If needed |
| chmod/chown | GRANTED | Permissions |
| mkdir/rmdir | GRANTED | Directory ops |
| systemctl | DENIED | Host services (ask first) |
| zfs commands | GRANTED | ZFS pool operations |
| wg commands | DENIED | WireGuard (ask first) |

### 3. NETWORK OPERATIONS ✓
| Operation | Permission | Notes |
|-----------|------------|-------|
| Port scanning (self) | GRANTED | Verification tests |
| API calls | GRANTED | Internal services |
| Pull Docker images | GRANTED | From public registries |
| DNS lookups | GRANTED | Setup only |

### 4. BUILD OPERATIONS ✓
| Operation | Permission | Scope |
|-----------|------------|-------|
| Extract embedded code | GRANTED | 32 files |
| Compile Rust code | GRANTED | MUSL target |
| Build Docker images | GRANTED | All services |
| Pull AI models | GRANTED | Ollama |
| Run test suite | GRANTED | verification.sh |
| Start/stop containers | GRANTED | docker compose |

---

## OPERATIONS REQUIRING EXPLICIT APPROVAL

The following operations will NOT be executed without explicit user approval:

| # | Operation | Reason |
|---|-----------|--------|
| 1 | `rm -rf /*` | Destructive - NEVER |
| 2 | WireGuard systemctl | Host network impact |
| 3 | ZFS destroy pool | Data destruction |
| 4 | Kill-switch execution | Emergency only |
| 5 | Offsite replication | Network configuration |
| 6 | Firewall changes | UFW/iptables |
| 7 | Host reboot | System interruption |

---

## AUTONOMOUS DECISION LIMITS

### Within 4-Hour Window:
1. **ALLOWED**: Create any file in /opt/aetheris/
2. **ALLOWED**: Build and test in isolation
3. **ALLOWED**: Run verification tests
4. **ALLOWED**: Pull public Docker images
5. **ALLOWED**: Compile Rust code
6. **ALLOWED**: Start/stop Docker containers
7. **ALLOWED**: Modify build scripts and configs

### NOT ALLOWED (Even in 4-Hour Window):
1. Modify any file outside /opt/aetheris/ (except /root/.aetheris_key)
2. Access user credentials or secrets
3. Push to any git remote
4. Make network calls to external services (except Docker Hub, Rust crates)
5. Install kernel modules
6. Modify /etc/wireguard/ without approval
7. Any destructive operations

---

## REPORTING REQUIREMENTS

During 4-hour build window, agent will report:

| Milestone | Report |
|-----------|--------|
| Phase 1 Complete | "✓ File extraction complete: X/32 files" |
| Phase 2 Complete | "✓ Rust build complete: Binary size: X MB" |
| Phase 3 Complete | "✓ Container build complete: X/6 images" |
| Phase 4 Complete | "✓ Host prep complete" |
| Phase 5 Complete | "✓ Tests: X/27 passed" |
| Blocked | "⚠️ BLOCKED: [reason] - awaiting approval" |
| Complete | "✓✓✓ AETHERIS BUILD COMPLETE" |

---

## EMERGENCY STOP

If ANY of the following occur, build stops immediately and user is notified:

1. Data loss detected
2. Build failure after 3 retries
3. Unexpected system changes
4. Permission errors
5. Security warnings

---

## APPROVAL CONFIRMATION

**User:** [To be confirmed]
**Timestamp:** [To be recorded]
**Window:** 4 hours from start
**Scope:** As defined above

**I confirm autonomous build operation is authorized for the scope defined in this document.**
