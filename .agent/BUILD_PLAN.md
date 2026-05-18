# AETHERIS + LLMVM - COMPLETE BUILD PLAN
## Build Log, Reasoning & Decision Trail

**Created:** 2026-05-01
**Status:** Phase 1 In Progress (awaiting Oracle + Cloudflare credentials)
**Domain:** aimlds.tech → ai.aimlds.tech

---

## EXECUTIVE SUMMARY

Aetheris is a sovereign AI-native personal cloud (Rust + WireGuard + OPA + ZFS).
It originally used Ollama for local AI inference. The user has LMStudio running locally
with 21 models loaded. The Oracle Cloud Always Free tier (4 OCPU / 24GB ARM VM) will
serve as the primary remote inference backend, tunneled through Cloudflare for zero-trust
access. This solves the local memory constraint where large models like phi-4-reasoning-plus
trigger LMStudio's memory guardrails.

**Goal:** Replace Ollama dependency with LMStudio (local + remote) via OpenAI-compatible API,
build a lightweight RAG framework (no LangChain/Haystack), and deploy everything on free-tier
infrastructure with $0/month cost.

---

## CREDENTIALED STATE

| Service | Account | Status |
|---------|---------|--------|
| Oracle Cloud | nrupalakolkar@gmail.com | ✅ Signed up, API keys pending |
| Cloudflare | nrupalakolkar@gmail.com | ✅ 45 domains, 2FA enabled, token pending |
| Azure | nrupalakolkar@hotmail.com | ✅ Signed up |
| AWS | nrupalakolkar@gmail.com | ❌ Not yet signed up |

**Domain selected:** `aimlds.tech` → `ai.aimlds.tech` for AI endpoint

---

## PHASE 0: SESSION START (What Was Done Before Plan)

### 0.1 Initial State Discovery
**What we found:**
- Aetheris project at `C:\Users\HomeUser\Downloads\Aetheris` — 50% complete
- 11 Docker containers running in emulation mode
- Ollama container in compose.yaml but never actually used
- User has LMStudio running locally on port 1234 with 21 models
- User has `multi-cloud-rag-free` GitHub repo (skeleton only, 10 files)
- User wants to use Oracle VM + Cloudflare Tunnel for remote inference

**Why this matters:** We were building for Ollama (containerized, needs port 11434, `/api/generate` endpoint)
when the user already has LMStudio running (OpenAI-compatible, port 1234, `/v1/chat/completions` endpoint).
This was a fundamental mismatch requiring code changes.

### 0.2 Port Conflict Discovery
**Problem:** WireGuard's linuxserver/wireguard image runs CoreDNS internally, which binds
to port 8080. Aetheris Core (Rust) also tried to bind to 8080. Result: `Address in use` error.

**Solution:** Changed Aetheris Core to use `PORT` environment variable (default 8888).
Updated `main.rs` and `sync.rs` to read `PORT` from env instead of hardcoded 8080.

**File changes:**
- `aetheris/core/src/main.rs` — port from env var
- `aetheris/core/src/sync.rs` — port from env var
- `aetheris/compose.emulation.yaml` — `PORT=8888`, healthcheck on 8888

### 0.3 API Migration: Ollama → LMStudio (OpenAI-compatible)
**Problem:** Original code used Ollama-specific endpoints:
- `/api/generate` (Ollama text generation)
- `/api/embeddings` (Ollama embedding)
- `/api/tags` (list models)

LMStudio uses OpenAI-compatible endpoints:
- `/v1/chat/completions` (chat/text generation)
- `/v1/embeddings` (embeddings)
- `/v1/models` (list models)

**Solution:** Rewrote connector and implementation files.

**File changes:**
- `aetheris/core/src/connector.rs` — added `ai_query()`, `list_models()`, switched to `/v1/` endpoints
- `aetheris/core/src/implementation.rs` — `OllamaBridge` → `AIBridge`, LMStudio healthcheck via `/v1/models`
- `aetheris/core/src/bridge.rs` — added `ModelBridge` trait with `query()` method
- `aetheris/sentinel/ai_sentinel.py` — switched to `/v1/chat/completions`, added `AI_MODEL` env var

**Key decision:** Made `ai_query()` accept optional model parameter with default `microsoft/phi-4-reasoning-plus`.
This gives the user ability to switch models at runtime without code changes.

### 0.4 Subnet Conflict Resolution
**Problem:** Docker network `10.0.10.0/24` conflicted with existing host network.

**Solution:** Changed to `10.88.99.0/24` — a less common range unlikely to conflict.

**File changes:**
- `aetheris/compose.emulation.yaml` — all 10.0.10.x → 10.88.99.x IPs

### 0.5 Static IP Removal
**Problem:** Hardcoded static IPs caused "Address already in use" when containers restarted.

**Solution:** Removed all `ipv4_address` assignments, switched to Docker DNS service names.

**File changes:**
- `aetheris/compose.emulation.yaml` — removed static IPs, updated env vars to use service names

### 0.6 OPA Healthcheck Fix
**Problem:** OPA runs in a scratch container with no shell. Healthcheck used `wget` which doesn't exist.

**Solution:** Removed healthcheck, changed `depends_on` to `service_started` instead of `service_healthy`.

### 0.7 Sentinel AI Endpoint Fix
**Problem:** Sentinel was hardcoded to `http://aetheris_ai:11434` — wrong host and port.

**Solution:** Changed to `http://host.docker.internal:1234` (LMStudio on host machine).

**Reasoning for `host.docker.internal`:** Docker Desktop on Windows provides this special DNS
name that resolves to the host machine's IP. This lets containers reach services running on
the host (LMStudio) without needing a containerized AI engine.

### 0.8 AI Endpoint Configuration
**Current setup:**
```
Aetheris Core → http://host.docker.internal:1234 (local LMStudio)
Sentinel      → http://host.docker.internal:1234 (local LMStudio)
```

**Target setup:**
```
Aetheris Core → https://ai.aimlds.tech (Oracle VM via Cloudflare)
Sentinel      → https://ai.aimlds.tech (Oracle VM via Cloudflare)
Fallback      → http://host.docker.internal:1234 (local LMStudio if remote unavailable)
```

---

## PHASE 1: ORACLE VM + CLOUDFLARE TUNNEL (Current Phase)

### 1.1 Oracle VM Provisioning

**Specs:**
- Shape: VM.Standard.A1.Flex (ARM64 / Ampere Altra)
- OCPUs: 4
- RAM: 24 GB
- Boot Volume: 50 GB (minimum 47 GB)
- Block Volume: 200 GB (Always Free limit)
- OS: Ubuntu 22.04 LTS ARM64
- Network: Private subnet (no public IP)
- Cost: $0/month (Always Free)

**Why ARM64?** Oracle's Always Free tier only offers ARM-based instances. LMStudio supports
ARM64 natively. All models must be GGUF format (which LMStudio handles automatically).

**Why no public IP?** Cloudflare Tunnel creates an outbound-only connection from the VM to
Cloudflare's edge. No inbound ports are open. This eliminates attack surface entirely.

**Terraform files created:**
- `LLMVM/main.tf` — full OCI infrastructure (VCN, subnet, instance, security list, route table)
- `LLMVM/variables.tf` — all configurable parameters
- `LLMVM/scripts/cloud-init.yaml` — first-boot provisioning script

**Cloud-init does:**
1. System update + security hardening (fail2ban, UFW, unattended-upgrades)
2. 16GB swap file + ZRAM optimization (critical for 24GB RAM with large models)
3. LMStudio ARM64 headless installation
4. Model download and preload
5. systemd service for LMStudio (auto-restart on crash)
6. Cloudflare Tunnel installation and configuration
7. SSH hardening (no root login, no password auth, max 3 attempts)

### 1.2 Cloudflare Tunnel Configuration

**Architecture:**
```
Aetheris Core ──HTTPS──▶ Cloudflare Edge ──Encrypted Tunnel──▶ Oracle VM (localhost:1234)
```

**Security layers:**
1. Cloudflare Access policy requires Bearer token in `Authorization` header
2. Tunnel is outbound-only — no inbound firewall rules needed
3. UFW on VM blocks all inbound except SSH (from OCI bastion only)
4. TLS termination at Cloudflare edge (full strict mode)

**Config files created:**
- `LLMVM/config/cloudflare-tunnel/config.yml` — ingress rules
- `LLMVM/docs/ORACLE_SETUP.md` — step-by-step provisioning guide
- `LLMVM/scripts/ai-connection-manager.sh` — endpoint selection script

**Ingress rules:**
```yaml
- hostname: ai.aimlds.tech
  service: http://localhost:1234
  originRequest:
    connectTimeout: 30s
- service: http_status:403  # deny everything else
```

### 1.3 Model Selection Strategy

**Available on user's LMStudio (verified working):**

| Model | Size | Status | Use Case |
|-------|------|--------|----------|
| `essentialai/rnj-1` | 5.1 GB | ✅ Tested, works | General chat, fast |
| `strand-rust-coder-14b-v1` | ~8 GB | ✅ Loaded | Code generation (Rust focus) |
| `microsoft/phi-4-reasoning-plus` | ~5 GB | ✅ Loaded | Reasoning, analysis |
| `nvidia/nemotron-3-nano-4b` | 2.8 GB | ✅ Tested, works | Fast responses, fallback |

**On Oracle VM (recommended):**
- **Primary:** `microsoft/phi-4-reasoning-plus` — best reasoning for Aetheris use cases
- **Fallback:** `nvidia/nemotron-3-nano-4b` — fast, low memory for health checks
- **Optional:** `strand-rust-coder-14b-v1` — code generation (heavy, preload only when needed)

**Why not load all models simultaneously?** Oracle VM has 24GB RAM. With 16GB swap and
model overhead, loading 2 models simultaneously is safe. 3+ will cause OOM kills.

**Solution:** Use `lms load` / `lms unload` to swap models dynamically based on request type.

### 1.4 Pending Actions (Awaiting Credentials)

1. User provides Oracle API credentials → `terraform.tfvars`
2. User provides Cloudflare API token → tunnel config
3. Run `terraform init && terraform apply` → VM provisioned
4. Cloud-init runs automatically (5-10 minutes)
5. Test: `curl https://ai.aimlds.tech/v1/models -H "Authorization: Bearer $KEY"`
6. Update Aetheris `AI_ENDPOINT` to `https://ai.aimlds.tech`

---

## PHASE 2: RAG FRAMEWORK (Building Now — No Credentials Needed)

### 2.1 Design Philosophy

**What we're NOT using:**
- ❌ LangChain — 200+ dependencies, opaque abstractions, vendor lock-in
- ❌ Haystack — heavy, requires specific orchestrator setup
- ❌ Milvus — overkill for personal use, needs Kubernetes or complex Docker setup
- ❌ Pinecone/Weaviate — cloud-hosted, breaks zero-cloud principle
- ❌ LlamaIndex — similar issues to LangChain

**What we ARE using:**
- ✅ `requests` — HTTP calls to LMStudio
- ✅ `numpy` — vector math (cosine similarity)
- ✅ `sqlite3` — stdlib, zero dependencies, fast enough for personal RAG
- ✅ `tiktoken` — accurate token counting (optional fallback to char-based)

**Total dependencies:** 3 (requests, numpy, tiktoken)

### 2.2 Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Document   │────▶│   Chunker    │────▶│   Embedder   │
│  (text/pdf)  │     │  (512 token) │     │ (LMStudio)   │
└──────────────┘     └──────────────┘     └──────┬───────┘
                                                  │
                                                  ▼
                                         ┌──────────────┐
                                         │ Vector Store │
                                         │  (SQLite +   │
                                         │   NumPy)     │
                                         └──────┬───────┘
                                                │
                                    ┌───────────┼───────────┐
                                    ▼           ▼           ▼
                              ┌────────┐ ┌────────┐ ┌────────┐
                              │Retrieve│ │ ReRank │ │Generate│
                              │(Top-K) │ │(score) │ │(LMStudio)│
                              └────────┘ └────────┘ └────────┘
```

### 2.3 File Structure (Created/In Progress)

```
LLMVM/rag_core/
├── __init__.py          # ✅ Package init
├── config.py            # ✅ Centralized configuration
├── chunker.py           # ✅ Document chunking (semantic boundaries)
├── embedder.py          # ⬜ LMStudio embedding calls
├── vector_store.py      # ⬜ SQLite + NumPy vector database
├── retriever.py         # ⬜ Top-K semantic search
├── generator.py         # ⬜ LMStudio chat with context injection
├── pipeline.py          # ⬜ Orchestration (ingest → retrieve → generate)
└── tests/
    ├── test_chunker.py  # ⬜ Chunking tests
    ├── test_embedder.py # ⬜ Embedding tests
    └── test_pipeline.py # ⬜ End-to-end tests
```

### 2.4 Chunking Strategy

**Priority order:**
1. Paragraph boundaries (`\n\n`) — natural semantic breaks
2. Sentence boundaries (`.!? `) — fallback for large paragraphs
3. Hard token split — last resort for single long sentences

**Parameters:**
- `CHUNK_SIZE=512` tokens (fits in most model context windows)
- `CHUNK_OVERLAP=64` tokens (prevents information loss at boundaries)

**Why 512?** The nomic-embed-text model performs optimally at 512-token chunks.
Larger chunks dilute embedding quality; smaller chunks lose context.

### 2.5 Vector Store Design

**Why SQLite + NumPy instead of Milvus?**
- Personal RAG = maybe 10,000-50,000 chunks max
- Cosine similarity on 768-dim vectors: ~50ms for 50K vectors in NumPy
- Milvus adds 2GB+ memory overhead, Docker complexity, gRPC dependencies
- SQLite is ACID-compliant, zero-config, and Aetheris already uses SQL patterns

**Schema:**
```sql
CREATE TABLE chunks (
    id INTEGER PRIMARY KEY,
    text TEXT NOT NULL,
    source TEXT,
    chunk_index INTEGER,
    token_count INTEGER,
    metadata TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE embeddings (
    chunk_id INTEGER PRIMARY KEY REFERENCES chunks(id),
    vector BLOB NOT NULL  -- stored as packed float32 bytes
);

CREATE INDEX idx_chunks_source ON chunks(source);
CREATE INDEX idx_chunks_created ON chunks(created_at);
```

### 2.6 Retrieval Strategy

1. **Embed query** via LMStudio `/v1/embeddings`
2. **Cosine similarity** against all stored vectors (NumPy)
3. **Filter** by `SIMILARITY_THRESHOLD` (default 0.65)
4. **Sort** by score descending
5. **Take Top-K** (default 5)
6. **Optional rerank** — cross-encoder if available (future enhancement)

### 2.7 Generation Strategy

**Prompt template:**
```
System: {system_prompt}

Context:
---
{chunk_1_text} (source: {source_1}, relevance: {score_1})
{chunk_2_text} (source: {source_2}, relevance: {score_2})
...
---

Question: {user_query}

Answer based on the context provided. Cite sources where applicable.
```

**Model routing:**
- Simple factual questions → `nvidia/nemotron-3-nano-4b` (fast)
- Complex reasoning → `microsoft/phi-4-reasoning-plus` (accurate)
- Code questions → `strand-rust-coder-14b-v1` (specialized)

---

## PHASE 3: AETHERIS INTEGRATION

### 3.1 AI Connection Manager

**Location:** `LLMVM/scripts/ai-connection-manager.sh` (already created)

**Logic:**
```
if local_lmstudio.available AND local_lmstudio.has_model(requested_model):
    return local_endpoint  # Fastest, no network latency

elif remote_ai.available AND remote_ai.has_api_key():
    return remote_endpoint  # Oracle VM via Cloudflare

elif local_lmstudio.available:
    return local_endpoint  # Use whatever local model is loaded

else:
    return error  # No AI endpoints available
```

**Integration into Aetheris:**
- Add `AI_FALLBACK_ENDPOINT` env var to Aetheris Core
- Core tries primary → fallback → error
- Sentinel uses same logic

### 3.2 Aetheris Core Changes Needed

**Current:** `connector.rs` calls single `ai_url`
**Target:** Try primary → fallback with automatic routing

**Implementation plan:**
```rust
pub struct AetherisConnector {
    pub primary_ai_url: String,      // https://ai.aimlds.tech
    pub fallback_ai_url: String,     // http://host.docker.internal:1234
    pub opa_url: String,
    pub vault_path: String,
    pub api_key: Option<String>,     // For remote endpoint
}

impl AetherisConnector {
    async fn ai_query_with_fallback(&self, prompt: &str, model: Option<&str>) -> Result<String, String> {
        // Try primary first
        match self.ai_query(prompt, model).await {
            Ok(response) => Ok(response),
            Err(_) => {
                // Fallback to local
                let client = reqwest::Client::new();
                // ... call fallback endpoint
            }
        }
    }
}
```

### 3.3 RAG Pipeline Integration

**New Aetheris API endpoints:**
- `POST /rag/ingest` — upload document → chunk → embed → store
- `GET /rag/search?q=...` — semantic search across indexed documents
- `POST /rag/ask` — full RAG: search → inject context → LLM answer
- `DELETE /rag/source/{source}` — remove document and its embeddings

**These require:**
- `rag_core/` accessible from Aetheris Core (Python subprocess or Rust port)
- Vector store mounted as volume
- Model availability check before embedding

**Decision:** Keep RAG as a separate service on Oracle VM. Aetheris Core calls it via API.
This maintains isolation principle (every interaction is an API call).

**RAG API on Oracle VM:**
```python
# FastAPI or simple HTTP server on Oracle VM
# Port: 8080 (internal), exposed via Cloudflare as rag.aimlds.tech

@app.post("/ingest")
async def ingest(document: Document):
    chunks = chunker.chunk(document.text)
    embeddings = embedder.embed([c.text for c in chunks])
    vector_store.add(chunks, embeddings)
    return {"status": "ok", "chunks": len(chunks)}

@app.get("/search")
async def search(q: str, k: int = 5):
    query_embedding = embedder.embed([q])[0]
    results = vector_store.search(query_embedding, top_k=k)
    return results

@app.post("/ask")
async def ask(question: str, model: str = None):
    results = search(question)
    context = build_context(results)
    answer = generator.generate(question, context, model)
    return {"answer": answer, "sources": results}
```

---

## PHASE 4: MULTI-CLOUD REDUNDANCY (Future)

### 4.1 Why Multi-Cloud?

The `multi-cloud-rag-free` repo name suggests ambition. But reality check:

| Cloud | Free VM | RAM | Can Run LLM? | Useful For |
|-------|---------|-----|--------------|------------|
| Oracle | A1.Flex 4 OCPU | 24 GB | ✅ Yes | Primary inference |
| GCP | e2-micro | 1 GB | ❌ No | Doc ingestion, metadata |
| Azure | B1s | 1 GB | ❌ No | Backup, monitoring |
| AWS | t4g.micro | 0.5 GB | ❌ No | S3 cold storage |

**Realistic multi-cloud RAG:**
- Oracle = AI brain (inference + vector DB)
- GCP = Document pre-processing (chunking, metadata extraction)
- Azure = Backup replica of vector index
- AWS = Raw document archive (S3)

**Not realistic:** Multiple LLM instances across clouds. Only Oracle has enough RAM.

### 4.2 GCP e2-micro Setup (Future)
- Region: us-central1 (free tier eligible)
- Purpose: Document ingestion worker
- Receives documents from Aetheris → chunks → sends to Oracle for embedding
- Uses GCP Pub/Sub for async processing

### 4.3 Azure B1s Setup (Future)
- Free for 12 months (750 hours/month)
- Purpose: Vector index backup
- Receives periodic SQLite dump from Oracle
- Serves read-only queries if Oracle is down

### 4.4 AWS t4g.micro Setup (Future)
- 12 months free (new accounts) or $200 credit
- Purpose: S3 cold storage for raw documents
- Lifecycle policy: move to Glacier after 90 days

---

## PHASE 5: MONITORING & PRODUCTION

### 5.1 Oracle VM Monitoring
- Prometheus + Grafana on Oracle VM (Docker)
- Metrics: LMStudio memory usage, model load times, request latency
- Alerts: OOM prediction, tunnel disconnection, model unload

### 5.2 Budget Alerts
- Oracle: Set budget alarm at $1/month (should never trigger)
- GCP: Billing alerts at $0.01
- Azure: Spending limit enforcement
- AWS: Billing alerts via SNS

### 5.3 Auto-Recovery
- systemd Restart=always for LMStudio
- Cloudflare Tunnel auto-reconnect
- Model preload on boot via cron
- Health check every 60 seconds

---

## DECISION LOG

### Decision 1: LMStudio over Ollama
**Date:** 2026-05-01
**Reason:** User already has LMStudio running with 21 models. OpenAI-compatible API is
more widely supported than Ollama's custom API. No need to pull and manage models separately.

### Decision 2: Oracle VM over GPU instances
**Date:** 2026-05-01
**Reason:** Oracle Always Free gives 24GB RAM ARM VM at $0. GPU instances cost $0.50+/hr.
GGUF quantized models (Q4_K_M) run well on CPU with 24GB. Cost efficiency wins.

### Decision 3: Cloudflare Tunnel over public IP
**Date:** 2026-05-01
**Reason:** Zero inbound ports = zero attack surface. Cloudflare provides TLS termination,
DDoS protection, WAF, and Access policies. Free tier includes tunnels.

### Decision 4: SQLite + NumPy over Milvus
**Date:** 2026-05-01
**Reason:** Personal RAG doesn't need distributed vector DB. SQLite is zero-config,
ACID-compliant, and fast enough for <100K vectors. Milvus adds 2GB+ overhead.

### Decision 5: No LangChain/Haystack
**Date:** 2026-05-01
**Reason:** These frameworks add 200+ dependencies for what is fundamentally:
chunk → embed → store → search → generate. Simple code is more maintainable,
more debuggable, and doesn't break when upstream dependencies update.

### Decision 6: domain aimlds.tech
**Date:** 2026-05-01
**Reason:** User has 45 domains. aimlds.tech is:
- Short and memorable
- Contains "ai" and "ml" and "ds" (AI/ML/DS)
- .tech TLD fits infrastructure project
- Active until 7/22/2026
- Not used for anything else currently

### Decision 7: RAG as separate service on Oracle VM
**Date:** 2026-05-01
**Reason:** Aetheris Core is Rust. RAG pipeline is Python (embeddings, vector math).
Keeping them separate maintains language boundaries, allows independent scaling,
and follows Aetheris isolation principle.

### Decision 8: ARM64-only infrastructure
**Date:** 2026-05-01
**Reason:** Oracle Always Free is ARM-only. All containers and binaries must be ARM64.
LMStudio provides ARM64 builds. All Docker images used must have linux/arm64 variants.

---

## RISK REGISTER

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Oracle capacity unavailable | High | Medium | Try multiple availability domains; use AMD micro as fallback |
| Cloudflare tunnel drops | High | Low | systemd Restart=always; health check + alert |
| OOM on Oracle VM | High | Medium | 16GB swap; ZRAM; model unload on idle; max 2 models loaded |
| LMStudio ARM64 bugs | Medium | Low | Pin specific version; test before deployment |
| Free tier policy changes | High | Low | Monitor Oracle announcements; have paid backup plan ($5/mo) |
| API key exposure | High | Low | Cloudflare Access policy; key rotation; never commit to git |
| Model download fails | Medium | Low | Pre-download; have fallback model; retry logic |

---

## FILE MANIFEST

### LLMVM Project
```
LLMVM/
├── README.md                          # ✅ Project overview
├── main.tf                            # ✅ Oracle Terraform
├── variables.tf                       # ✅ Terraform variables
├── .gitignore                         # ⬜ Credentials, .env, *.tfstate
├── config/
│   └── cloudflare-tunnel/
│       └── config.yml                 # ✅ Tunnel ingress rules
├── scripts/
│   ├── cloud-init.yaml                # ✅ VM first-boot setup
│   └── ai-connection-manager.sh       # ✅ Endpoint selector
├── docs/
│   └── ORACLE_SETUP.md               # ✅ Provisioning guide
├── rag_core/
│   ├── __init__.py                    # ✅ Package init
│   ├── config.py                      # ✅ Configuration
│   ├── chunker.py                     # ✅ Document chunking
│   ├── embedder.py                    # ⬜ Embedding client
│   ├── vector_store.py               # ⬜ SQLite vector DB
│   ├── retriever.py                  # ⬜ Semantic search
│   ├── generator.py                  # ⬜ LLM generation
│   ├── pipeline.py                   # ⬜ Orchestration
│   └── tests/                         # ⬜ Test suite
└── .agent/
    └── BUILD_PLAN.md                  # ✅ This file
```

### Aetheris Changes
```
aetheris/
├── core/src/
│   ├── main.rs                        # ✅ PORT env var
│   ├── connector.rs                   # ✅ OpenAI API + list_models()
│   ├── implementation.rs             # ✅ LMStudio bridge
│   └── bridge.rs                      # ✅ ModelBridge trait
├── sentinel/
│   └── ai_sentinel.py                 # ✅ LMStudio API + loop
├── compose.emulation.yaml             # ✅ LMStudio endpoint, no Ollama
├── config/
│   ├── oracle-vm/
│   │   └── setup.sh                   # ✅ Moved to LLMVM
│   └── cloudflare-tunnel/
│       └── config.yml                 # ✅ Moved to LLMVM
├── TODO.md                            # ✅ Updated progress
└── .agent/
    └── BUILD_PLAN.md                  # ✅ This file (symlink)
```

---

## NEXT ACTIONS (Priority Order)

1. **User provides credentials** → Oracle API key + Cloudflare token
2. **Create terraform.tfvars** → Fill in OCIDs, keys, domain
3. **terraform apply** → Provision Oracle VM (10-15 min)
4. **Wait for cloud-init** → LMStudio + Tunnel setup (5-10 min)
5. **Test remote endpoint** → curl ai.aimlds.tech
6. **Update Aetheris config** → Point to remote endpoint
7. **Build RAG core** → embedder, vector_store, retriever, generator, pipeline
8. **Test RAG pipeline** → ingest → search → ask
9. **Integration tests** → Aetheris ↔ Oracle VM ↔ Cloudflare
10. **Documentation** → Update ARCHITECTURE.md, user guide

---

**Last Updated:** 2026-05-01 12:34 PM
**Next Update:** When credentials are provided or Phase 2 RAG core is complete
