# AETHERIS - BUILD REQUIREMENTS
## v1.0 - Production Release

---

## 1. HOST REQUIREMENTS

### Minimum Specifications
| Component | Requirement |
|-----------|-------------|
| CPU | 4 cores |
| RAM | 8 GB |
| Storage | 500 GB (ZFS vault) |
| Network | Static IP or DDNS |
| OS | Ubuntu 22.04 LTS / Debian 12 |

### Required Kernel Modules
- ZFS (Native)
- WireGuard (kernel module)

---

## 2. SYSTEM DEPENDENCIES

### OS Packages (apt)
```bash
# Core
apt install -y zfsutils-linux wireguard docker.io

# Build tools
apt install -y build-essential musl-tools

# Optional
apt install -y ufw curl wget git
```

### Rust Toolchain
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add x86_64-unknown-linux-musl
```

---

## 3. CONTAINER IMAGES

| Image | Source | Tag |
|-------|--------|-----|
| linuxserver/wireguard | Docker Hub | latest |
| ollama/ollama | Docker Hub | latest |
| chromadb/chroma | Docker Hub | latest |
| victoriametrics/victoria-metrics | Docker Hub | latest |
| openpolicyagent/opa | Docker Hub | latest |
| gcr.io/distroless/static-debian12 | Google Container Registry | latest |
| alpine | Docker Hub | latest |

---

## 4. RUST DEPENDENCIES (Cargo.toml)

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["io"] }
axum = "0.7"
tower-http = { version = "0.5", features = ["fs"] }
notify = "6.1"
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
prometheus = "0.13"
lazy_static = "1.4"
async-trait = "0.1"
```

---

## 5. ENVIRONMENT VARIABLES

| Variable | Default | Purpose |
|----------|---------|---------|
| `PUID` | 1000 | Podman user ID |
| `PGID` | 1000 | Podman group ID |
| `TZ` | Etc/UTC | Timezone |
| `SERVERURL` | auto | WireGuard public IP |
| `PEERS` | mobile,laptop | WireGuard peers |
| `IS_PERSISTENT` | TRUE | ChromaDB persistence |
| `AI_ENDPOINT` | http://ai-engine:11434 | Ollama URL |
| `DB_ENDPOINT` | http://vector-db:8000 | ChromaDB URL |

---

## 6. PORT ASSIGNMENTS

| Service | Internal IP | Port | Protocol |
|---------|-------------|------|----------|
| WireGuard | Host IP | 51820 | UDP |
| Aetheris Core | 10.0.0.1 | 8080 | TCP |
| Ghost Shell | 10.0.0.1 | 8081 | TCP |
| OPA | 10.0.0.1 | 8181 | TCP |
| Ollama | 10.0.0.1 | 11434 | TCP |
| ChromaDB | 10.0.0.1 | 8000 | TCP |
| VictoriaMetrics | 10.0.0.1 | 8428 | TCP |
| Grafana | 10.0.0.1 | 3000 | TCP |

---

## 7. FILESYSTEM STRUCTURE

```
/opt/aetheris/
├── bootstrap.sh
├── compose.yaml
├── Dockerfile.core
├── core/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── sync.rs
│   │   ├── connector.rs
│   │   ├── bridge.rs
│   │   ├── implementation.rs
│   │   ├── metrics.rs
│   │   └── watcher.rs
│   └── ui/
│       └── index.html
├── config/
│   ├── wireguard/
│   │   └── wg0.conf
│   └── policy/
│       ├── policy.rego
│       └── opa_conf.yaml
├── scripts/
│   ├── bootstrap.sh
│   ├── verification.sh
│   ├── killswitch.sh
│   ├── vault_setup.sh
│   ├── vault_sync.sh
│   ├── vault_rollback.sh
│   ├── id_tracker.sh
│   └── ghost/
│       └── simulated_ui.sh
├── data/
│   ├── ollama/
│   ├── chroma/
│   └── victoria/
├── vault/           (ZFS mount point)
├── sentinel/
│   └── ai_sentinel.py
├── monitoring/
│   ├── zrepl.yml
│   └── sentinel_dashboard.json
├── docker-compose.ghost.yaml
├── state.json
└── tests/
    └── tests.json

/root/.aetheris_key       (Master encryption key - 32 bytes hex)
/etc/wireguard/wg0.conf    (WireGuard server config)
```

---

## 8. ZFS REQUIREMENTS

| Parameter | Value |
|-----------|-------|
| Pool Name | aetheris_vault |
| Dataset | aetheris_vault/secure_data |
| Encryption | AES-256-GCM |
| Key Format | hex |
| Compression | lz4 |
| Key File | /root/.aetheris_key |

---

## 9. AI MODELS

| Model | Purpose | Size |
|-------|---------|------|
| nomic-embed-text | Semantic embeddings | ~274 MB |
| mistral | General AI inference | ~4.1 GB |

---

## 10. SECURITY REQUIREMENTS

### Zero-Trust Policy
- Default: Deny all
- Authentication: JWT from local WireGuard peer
- Authorization: OPA policy evaluation
- Auto-ban: 5 failures = 1 hour ban

### Kill-Switch Triggers
- Physical seizure
- Remote compromise
- Dead man's switch (optional cron)

---

## 11. TEST REQUIREMENTS

### 27 Automated Tests Required
- Network: 3 tests
- Security: 6 tests
- Storage: 4 tests
- AI: 5 tests
- Monitoring: 4 tests
- Core API: 8 tests

### Success Criteria
- All tests pass
- Zero exposed ports except 51820/UDP
- ZFS encryption verified
- Semantic search functional

---

## 12. OFFSITE BACKUP REQUIREMENTS

| Method | Protocol | Frequency |
|--------|----------|-----------|
| SSH ZFS Send | zfs send over SSH | Hourly |
| USB Cold Storage | Manual backup | Weekly |

---

## 13. CLIENT REQUIREMENTS

| Platform | App | Protocol |
|----------|-----|----------|
| Android | WireGuard | UDP 51820 |
| iOS | WireGuard | UDP 51820 |
| Windows | WireGuard | UDP 51820 |
| macOS | WireGuard | UDP 51820 |
| Linux | WireGuard | UDP 51820 |

---

**REQUIREMENTS VERSION:** 1.0
**STATUS:** APPROVED
**DATE:** 2026-04-15
