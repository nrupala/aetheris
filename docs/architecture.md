# AETHERIS - ARCHITECTURE SPECIFICATION
## v1.0

---

## 1. SYSTEM OVERVIEW

**Project Name:** Aetheris (Project Zero-Cloud)
**Type:** Sovereign AI-Native Personal Cloud
**Architecture:** Containerized Zero-Trust Mesh
**Security Model:** Zero-Trust, Zero-Knowledge, Zero-Cloud

### Core Principles
1. **Invisibility**: Only UDP 51820 exposed to public internet
2. **Encryption**: E2EE with keys never leaving physical hardware
3. **Intelligence**: Local AI for semantic file understanding
4. **Resilience**: ZFS snapshots + offsite replication
5. **Isolation**: Every interaction is an API call

---

## 2. NETWORK TOPOLOGY

```
┌─────────────────────────────────────────────────────────────┐
│                    PUBLIC INTERNET                          │
│              (Only UDP 51820 visible)                      │
│                 Invisible to port scans                     │
└─────────────────────────┬───────────────────────────────────┘
                          │ WireGuard Handshake
                          │ (Curve25519 + ChaCha20-Poly1305)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    WIREGUARD MESH                           │
│                  Subnet: 10.0.0.0/24                        │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐   │
│  │   Mobile    │    │   Desktop   │    │   Server    │   │
│  │  (Peer)     │    │  (Peer)     │    │  (Hub)      │   │
│  │ 10.0.0.2   │◄──►│ 10.0.0.3   │◄──►│ 10.0.0.1   │   │
│  └─────────────┘    └─────────────┘    └─────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. COMPONENT ARCHITECTURE

### 3.1 Aetheris Core (Rust)
```
┌─────────────────────────────────────────┐
│           AETHERIS CORE                │
│         (Rust Orchestrator)             │
├─────────────────────────────────────────┤
│  ┌───────────┐  ┌───────────┐          │
│  │   File    │  │   Zero-  │          │
│  │  Watcher  │  │   Trust   │          │
│  │ (inotify) │  │   Guard   │          │
│  └─────┬─────┘  └─────┬─────┘          │
│        │              │                 │
│        ▼              ▼                 │
│  ┌─────────────────────────────┐       │
│  │      AXUM HTTP ROUTER       │       │
│  │   /upload  /download        │       │
│  │   /search  /status          │       │
│  │   /metrics                  │       │
│  └─────────────────────────────┘       │
│                │                       │
│        ┌───────┴───────┐              │
│        ▼               ▼              │
│  ┌───────────┐  ┌───────────┐        │
│  │  Upload   │  │ Semantic  │        │
│  │  Handler  │  │  Indexer  │        │
│  └───────────┘  └───────────┘        │
└─────────────────────────────────────────┘
```

### 3.2 Container Stack
```
┌─────────────────────────────────────────────────────────────┐
│                    AETHERIS MESH                            │
│                 (WireGuard - UDP 51820)                     │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   CORE       │    │   AI ENGINE  │    │  VECTOR DB   │
│   (Rust)     │    │  (Ollama)    │    │  (ChromaDB)  │
│              │◄──►│              │◄──►│              │
│              │    │              │    │              │
│  Port: 8080  │    │  Port: 11434 │    │  Port: 8000  │
└──────────────┘    └──────────────┘    └──────────────┘
        │                                             │
        ▼                                             ▼
┌──────────────┐                            ┌──────────────┐
│  OPA POLICY  │                            │ ZFS VAULT    │
│  (Zero-Trust)│                            │ (Encrypted)  │
│  Port: 8181  │                            │ AES-256-GCM  │
└──────────────┘                            └──────────────┘
        │
        ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   METRICS    │    │   SENTINEL   │    │   GHOST      │
│  (Victoria)  │    │   (AI)       │    │   (Honeypot) │
│  Port: 8428 │    │              │    │  Port: 8081  │
└──────────────┘    └──────────────┘    └──────────────┘
```

---

## 4. DATA FLOW ARCHITECTURE

### 4.1 File Upload Flow
```
Client ──POST /upload──► Aetheris Core ──► OPA (authorize)
                                    │
                              (if allowed)
                                    │
                                    ▼
                              ZFS Vault (encrypted)
                                    │
                                    ▼
                              inotify trigger
                                    │
                                    ▼
                              Ollama (embed)
                                    │
                                    ▼
                              ChromaDB (index)
                                    │
                                    ▼
                              zrepl (snapshot)
```

### 4.2 Semantic Search Flow
```
Client ──GET /search?q=──► Aetheris Core ──► OPA (authorize)
                                     │
                               (if allowed)
                                     │
                                     ▼
                               Ollama (encode query)
                                     │
                                     ▼
                               ChromaDB (vector search)
                                     │
                                     ▼
                               Return results
```

---

## 5. SECURITY ARCHITECTURE

### 5.1 Zero-Trust Model
```
┌─────────────────────────────────────────────────────────────┐
│                    REQUEST INGRESS                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    LAYER 1: NETWORK                         │
│              WireGuard Handshake Verification                │
│              - Curve25519 key exchange                      │
│              - ChaCha20-Poly1305 encryption                │
│              - Peer ID validation                           │
└─────────────────────────────────────────────────────────────┘
                              │ (pass)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    LAYER 2: IDENTITY                        │
│              JWT Token Verification (OPA)                   │
│              - Short-lived JWT from local node             │
│              - Public key validation                        │
│              - Peer ID binding                             │
└─────────────────────────────────────────────────────────────┘
                              │ (pass)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    LAYER 3: AUTHORIZATION                  │
│              OPA Policy Evaluation                         │
│              - Role-based access (admin, analyst, etc.)    │
│              - Action-specific permissions                  │
│              - Semantic sensitivity check                   │
└─────────────────────────────────────────────────────────────┘
                              │ (pass)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    LAYER 4: RESOURCE                       │
│              File/Vault Access                              │
│              - Path traversal prevention                    │
│              - ZFS encryption verification                  │
│              - Audit logging                               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │   GRANT ACCESS   │
                    └─────────────────┘
```

### 5.2 Auto-Ban Logic
```
Failure Threshold: 5 attempts
Ban Duration: 1 hour
Ban Criteria: Peer ID (not IP address)
```

### 5.3 Ghost Shell (Honeypot)
```
┌─────────────────────────────────────────────────────────────┐
│              SUSPICIOUS PEER DETECTED                       │
│              (Failed auth / Anomalous behavior)            │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    GHOST PROXY                             │
│              Redirect to Honeypot Container                │
│              Port: 8081                                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    GHOST PLANE                             │
│              Isolated Alpine container                     │
│              - Canary tokens (fake passwords.txt)           │
│              - Simulated restricted shell                  │
│              - All actions logged                           │
│              - NO access to real vault                     │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. STORAGE ARCHITECTURE

### 6.1 ZFS Pool Structure
```
┌─────────────────────────────────────────────────────────────┐
│                    ZFS POOL: aetheris_vault                 │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────┐   │
│  │  DATASET: aetheris_vault/secure_data                │   │
│  │  - Encryption: AES-256-GCM                         │   │
│  │  - Key Format: hex (32 bytes)                      │   │
│  │  - Compression: lz4                                 │   │
│  │  - Mount: /opt/aetheris/vault                      │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  SNAPSHOTS: @zrepl_*                                │   │
│  │  - Interval: 15 minutes                            │   │
│  │  - Retention:                                       │   │
│  │    - Last 4 (15-minutely)                          │   │
│  │    - Last 24 (hourly)                              │   │
│  │    - Last 30 (daily)                               │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Backup Architecture
```
┌──────────────────┐         ┌──────────────────┐
│   LOCAL VAULT    │ ──────► │  OFFSITE NODE    │
│  (Primary ZFS)   │  zfs    │  (Encrypted      │
│                  │  send   │   replication)   │
└──────────────────┘   SSH   └──────────────────┘
       │
       │ zfs send
       ▼
┌──────────────────┐
│  USB COLD STORAGE │
│  (Air-gapped)     │
└──────────────────┘
```

---

## 7. API SPECIFICATION

### 7.1 Core Endpoints

#### POST /upload
```yaml
Request:
  Method: POST
  Content-Type: multipart/form-data
  Body:
    - file: (binary)
Response:
  200: {"status": "uploaded", "filename": "xxx"}
  403: {"error": "forbidden"}
```

#### GET /download/:filename
```yaml
Request:
  Method: GET
  Path: /download/:filename
Response:
  200: (binary stream)
  403: {"error": "forbidden"}
  404: {"error": "not found"}
```

#### GET /search
```yaml
Request:
  Method: GET
  Query: ?q=semantic query
Response:
  200:
    {
      "results": [
        {"filename": "doc.pdf", "score": 0.95, "excerpt": "..."}
      ]
    }
```

#### GET /status
```yaml
Response:
  200:
    {
      "vault": "encrypted_mounted",
      "mesh": "active",
      "peers": 3,
      "ai": "ready"
    }
```

#### GET /metrics
```yaml
Response:
  200: (Prometheus text format)
  aetheris_vault_usage_bytes 12345678
  aetheris_security_violations_total 0
```

### 7.2 OPA Endpoints

#### POST /v1/data/aetheris/authz/allow
```yaml
Request:
  Body:
    {
      "input": {
        "token": "eyJhbGci...",
        "user_role": "admin",
        "method": "GET",
        "action": "read_vault",
        "peer_id": "wg-peer-01"
      }
    }
Response:
  200: {"result": true}
  200: {"result": false}
```

### 7.3 Ollama Endpoints

#### POST /api/generate
```yaml
Request:
  Body:
    {
      "model": "mistral",
      "prompt": "What is Aetheris?",
      "stream": false
    }
Response:
  200: {"response": "Aetheris is..."}
```

#### POST /api/embeddings
```yaml
Request:
  Body:
    {
      "model": "nomic-embed-text",
      "prompt": "file content here"
    }
Response:
  200: {"embedding": [0.123, -0.456, ...]}
```

---

## 8. DIRECTORY STRUCTURE

```
/opt/aetheris/
├── core/                      # Rust orchestrator
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
├── vault/                     # ZFS mount point
├── sentinel/
│   └── ai_sentinel.py
├── monitoring/
│   ├── zrepl.yml
│   └── sentinel_dashboard.json
├── docker-compose.yaml
├── docker-compose.ghost.yaml
├── Dockerfile.core
├── state.json
└── tests/
    └── tests.json

/root/.aetheris_key           # 32-byte hex encryption key
/etc/wireguard/wg0.conf       # WireGuard server config
```

---

## 9. TECHNOLOGY VERSIONS

| Component | Version | Notes |
|-----------|---------|-------|
| Rust | 1.75+ | MUSL static target |
| Docker | Latest | ZFS storage driver |
| ZFS | 2.1+ | Native encryption |
| WireGuard | Latest | Kernel module |
| OPA | Latest | v1.x API |
| Ollama | Latest | Local AI |
| ChromaDB | Latest | Vector DB |
| VictoriaMetrics | Latest | Time-series |
| zrepl | Latest | Snapshotting |
| Distroless | debian12 | Minimal base |

---

**ARCHITECTURE VERSION:** 1.0
**STATUS:** APPROVED
**DATE:** 2026-04-15
