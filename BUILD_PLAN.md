# AETHERIS - SOVEREIGN AI-NATIVE PERSONAL CLOUD
## Production Build Plan v1.0

**Project Status:** APPROVED FOR BUILD
**Architecture Type:** Containerized Zero-Trust Mesh
**Security Philosophy:** Deceptive, Persistent, and Air-Gapped Intelligence
**Target Platform:** Containerized Bare Metal Emulation
**Build Mode:** API-First, Zero-Trust, Zero-Knowledge

---

## 1. CORE TECHNICAL STACK

| Layer | Technology | Version | Purpose |
|-------|------------|---------|---------|
| **Runtime** | Rust | 1.75+ | Static MUSL binary orchestrator |
| **Container** | Docker/Podman | Latest | Containerized deployment |
| **Base Image** | Google Distroless | static-debian12 | Minimal attack surface |
| **Networking** | WireGuard | UDP 51820 | L3 encrypted mesh tunnel |
| **Security** | Open Policy Agent (OPA) | Latest | Zero-trust authorization |
| **Persistence** | ZFS | AES-256-GCM | Native encrypted filesystem |
| **Snapshot** | zrepl | Latest | Automated ZFS snapshots |
| **AI Inference** | Ollama | Latest | Local LLM inference |
| **Vector DB** | ChromaDB | Latest | Semantic file indexing |
| **Metrics** | VictoriaMetrics | Latest | Time-series database |
| **Sync** | Syncthing | Latest | Cross-platform file sync |

---

## 2. SYSTEM ARCHITECTURE

```
┌─────────────────────────────────────────────────────────────┐
│                    PUBLIC INTERNET                          │
│              (Only UDP 51820 visible)                      │
└─────────────────────────┬───────────────────────────────────┘
                          │ WireGuard Handshake
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    WIREGUARD MESH                           │
│               (10.0.0.0/24 - Zero-Trust)                  │
└─────────────────────────────────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        ▼                 ▼                 ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│   AETHERIS    │ │   AI ENGINE   │ │  VECTOR DB    │
│   CORE        │ │  (Ollama)     │ │  (ChromaDB)   │
│   (Rust)      │◄─┤               │◄─┤               │
│   :8080       │ │   :11434      │ │   :8000       │
└───────────────┘ └───────────────┘ └───────────────┘
        │                                         │
        ▼                                         ▼
┌───────────────┐                         ┌───────────────┐
│  OPA POLICY   │                         │ ZFS VAULT     │
│  (Zero-Trust)│                         │ (Encrypted)   │
│  :8181       │                         │ AES-256-GCM   │
└───────────────┘                         └───────────────┘
```

---

## 3. API CALL MATRIX

Every interaction is an API call by default.

| Component | Endpoint | Method | Purpose |
|-----------|----------|--------|---------|
| **Aetheris Core** | `http://aetheris-core:8080` | REST | Main API Gateway |
| ↳ Upload | `/upload` | POST | File ingestion |
| ↳ Download | `/download/:filename` | GET | File retrieval |
| ↳ Search | `/search?q=` | GET | Semantic query |
| ↳ Status | `/status` | GET | System health |
| ↳ Metrics | `/metrics` | GET | Prometheus data |
| **OPA** | `http://opa:8181` | REST | Authorization |
| ↳ Decision | `/v1/data/aetheris/authz/allow` | POST | Access decision |
| **Ollama** | `http://ollama:11434` | REST | AI Inference |
| ↳ Generate | `/api/generate` | POST | LLM response |
| ↳ Embed | `/api/embeddings` | POST | Vector encoding |
| **ChromaDB** | `http://chroma:8000` | REST | Vector storage |
| ↳ Query | `/api/v1/collections/aetheris/query` | POST | Semantic search |
| **VictoriaMetrics** | `http://victoria:8428` | REST | Time-series |
| ↳ Query | `/api/v1/query` | GET | Metrics query |

---

## 4. DEPLOYMENT TARGETS

| Target | Platform | Status |
|--------|---------|--------|
| **Primary** | Bare Metal + Docker + ZFS | APPROVED |
| **Development** | Docker Desktop | APPROVED |
| **CI/CD** | GitHub Actions | APPROVED |

---

## 5. BUILD PHASES

| Phase | Duration | Status |
|-------|----------|--------|
| Phase 1: File Extraction | Day 1 | PENDING |
| Phase 2: Rust Core Build | Day 2-3 | PENDING |
| Phase 3: Container Stack | Day 4-5 | PENDING |
| Phase 4: Integration Tests | Day 6 | PENDING |
| Phase 5: Client Deployment | Day 7 | PENDING |
| **PRODUCTION** | Day 8+ | READY |

---

## 6. SECURITY FEATURES

- **Ghost Shell**: High-interaction honeypot (Honeypot Plane)
- **Sentinel**: AI analytics agent with VictoriaMetrics
- **Kill-Switch**: Scorched Earth Protocol for emergencies
- **Zero-Trust**: OPA-based authorization, no default permits
- **ZFS Encryption**: AES-256-GCM with native key management
- **WireGuard Stealth**: No ICMP response, invisible to port scans

---

## 7. EMERGENCY CONTACTS & RECOVERY

| Document | Purpose |
|----------|---------|
| `Emergency Recovery Manual (ERM).txt` | Total hardware failure recovery |
| `This is the Scorched Earth Protocol.txt` | Kill-switch activation |
| `Verification and testing.txt` | Integrity validation |

---

**APPROVED FOR BUILD**
**Autonomous Operation: 4 Hours**
**Last Updated:** 2026-04-15
