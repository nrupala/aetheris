# 🌌 Aetheris: The Sovereign AI-Native Cloud (Final Manifest)

**Project Status:** Deployment Ready
**Architecture Type:** Containerized Zero-Trust Mesh
**Security Philosophy:** Deceptive, Persistent, and Air-Gapped Intelligence

---

## 1. Core Technical Stack
- **Mesh Layer**: WireGuard (FOSS L3 Encrypted Tunnel) on UDP 51820.
- **Orchestration**: Rust-based Core (Static MUSL Binary) in a Distroless container.
- **Security Engine**: Open Policy Agent (OPA) for logic-based authorization.
- **Persistence**: Native ZFS Encryption (AES-256-GCM) with `zrepl` snapshotting.
- **Intelligence**: Local-only Ollama (Inference) + ChromaDB (Vector Search).
- **Frontend**: Zero-JS, Server-Side Rendered (SSR) HTML for XSS immunity.

---

## 2. Integrated Security Features
- **The Ghost Shell**: A high-interaction honeypot (Ghost Plane) that transparently traps suspicious Peer IDs into an isolated, dummy workspace.
- **The Sentinel**: A VictoriaMetrics-backed analytics bridge that uses AI to predict hardware failure or brute-force patterns.
- **The Kill-Switch**: A "Scorched Earth" script that shreds local keys, purges kernel memory, and unmounts the vault in seconds.

---

## 3. Key Operational Files
### /bootstrap.sh
Initializes directories, generates the master ZFS hex key, and ignites the container stack.

### /compose.yaml
Defines the multi-container organism (Core, AI, Mesh, Stats, Ghost).

### /policy.rego
The Zero-Trust gatekeeper. No request is processed without a verified Peer ID and OPA clearance.

### /state.json & /config.yaml
Separated machine-state and human-readable orchestration instructions for the Sync Bridge.

---

## 4. Emergency Recovery Protocol (ERM-01)
1. **Restore Hardware**: Fresh Linux + ZFS + Docker install.
2. **Re-Inject Key**: Manually recreate `/root/.aetheris_key` from offsite hex backup.
3. **Unlock Vault**: `zfs load-key` and `zfs mount`.
4. **Re-Synchronize**: Execute incremental `zfs receive` from offsite snapshot storage.

---

## 5. Port & Subnet Mapping

| Service | Internal IP | Port | Scope |
| :--- | :--- | :--- | :--- |
| WireGuard | Host IP | 51820/UDP | Public (Stealth) |
| Aetheris Core | 10.0.0.1 | 8080 | Private Mesh |
| Ghost Shell | 10.0.0.1 | 8081 | Internal Isolation |
| Metrics (VM) | 10.0.0.1 | 8428 | Management |

---

## 6. System Verification Hashes
Verify these against your source code to ensure zero tampering:
- `main.rs`: (Calculated upon compilation)
- `policy.rego`: (Calculated upon deployment)
