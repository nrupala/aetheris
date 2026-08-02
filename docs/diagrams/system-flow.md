# Aetheris System Flow

## Legend
- 🟢 Working
- 🟡 Partial / Broken
- 🔴 Not Implemented

```mermaid
flowchart TB
    subgraph Browser["🌐 Browser (You)"]
        DB[("Dashboard (/web/)")]
        AI[("AI Chat (/web/ai/)")]
        AG[("Agents (/web/agents/)")]
        RAG[("RAG (/web/rag/)")]
        DEV[("Dev Sandbox (/web/dev/)")]
    end

    subgraph Core["🟢 Rust Core — Port 8080"]
        GW[("API Gateway")]
        HLTH[("🟢 /health")]
        STS[("🟢 /status")]
        
        subgraph DevAPI["Dev Sandbox API"]
            LOGS[("🔴 /api/dev/logs ❌ 404")]
            CFG[("🔴 /api/dev/config ❌ 404")]
            METR[("🔴 /api/dev/metrics ❌ 404")]
        end

        subgraph AgentAPI["Agent API"]
            ASTAT[("🟢 /agents/status")]
            ATASK[("🟢 /tasks")]
            AWRK[("🟢 /workflow/run")]
            A2A[("🟢 /a2a/messages")]
            MCP[("🟢 /mcp/tools")]
        end

        subgraph OrchestratorAPI["Orchestrator API"]
            OST[("🟢 /orchestrator/state")]
            OFC[("🟢 /orchestrator/forecast")]
            CCKT[("🟢 /coordinator/circuits")]
            KGE[("🟢 /knowledge-graph/*")]
        end

        subgraph RAGAPI["RAG API"]
            QRY[("🟡 /query → mock data")]
            SRC[("🟡 /sources → empty")]
            ING[("🔴 /ingest/file → no backend")]
            STAT[("🟡 /stats → zeroes")]
        end

        subgraph AIAPI["AI API"]
            MODL[("🟡 /v1/models → hardcoded fallback")]
        end
    end

    subgraph Python["🟡 Python Orchestrator — Port 9090"]
        RAGENG[("🔴 RAG Engine")]
        KGENG[("🔴 Knowledge Graph")]
    end

    subgraph Ollama["🟡 Ollama — 127.0.0.1:11434 (loopback)"]
        LLM[("🔴 qwen2.5:7b")]
        EMBED[("🔴 Embedding Model")]
    end

    subgraph Infra["🟢 Infrastructure"]
        CFD[("cloudflared (native)")]
        CF[("Cloudflare Tunnel + Access")]
    end

    %% Connections that work
    DB --> GW
    AI --> GW
    AG --> GW
    RAG --> GW
    DEV --> GW

    GW --> HLTH
    GW --> STS
    GW --> ASTAT
    GW --> ATASK
    GW --> AWRK
    GW --> A2A
    GW --> MCP
    GW --> OST
    GW --> OFC
    GW --> CCKT
    GW --> KGE

    %% Broken connections
    GW -.-x LOGS
    GW -.-x CFG
    GW -.-x METR
    GW -.-> QRY
    GW -.-> SRC
    GW -.-x ING
    GW -.-> STAT
    GW -.-> MODL

    %% Backends that are not running
    QRY -.->|"❌ Not Connected"| RAGENG
    ING -.->|"❌ Not Connected"| RAGENG
    MODL -.->|"❌ Not Connected"| LLM

    %% Production path (native): cloudflared → loopback core, gated by Access
    CFD --> CF
    CFD --> GW
```

## Data Flow Diagram

```mermaid
flowchart LR
    subgraph Local["🖥️ Local Dev (current state)"]
        YOU["You in Browser"]
        CORE["Rust Core :8080"]
        MOCK[("🔴 Mock/Synthetic Data")]
    end

    subgraph Production["☁️ Production (target state)"]
        PROXY["Cloudflare Access + Tunnel → cloudflared"]
        RUST["aetheris-core :8080 (systemd, loopback)"]
        PYTHON["Python Orchestrator :9090"]
        OLLAMA["Ollama 127.0.0.1:11434"]
        VECDB[("ChromaDB :8000")]
        METRICS[("VictoriaMetrics :8428")]
    end

    YOU -->|"🟢 GET /"| CORE
    YOU -->|"🟢 GET /health"| CORE
    YOU -->|"🔴 GET /api/dev/*"| CORE
    CORE -->|"🟡 Returns mock"| MOCK

    YOU -.->|"🔴 Not Tested"| PROXY
    PROXY -.->|"🔴 Not Tested"| RUST
    RUST -.->|"🔴 Not Wired"| PYTHON
    RUST -.->|"🔴 Not Wired"| OLLAMA
    PYTHON -.->|"🔴 Not Wired"| VECDB
```

## What's Working vs What's Missing

### 🟢 Working Right Now
- Rust core compiles with 0 errors, 0 warnings
- 78/78 tests pass
- All server routes register and respond
- Agent pipeline (Planner→Researcher→Coder→Reviewer) compiles
- A2A messaging, MCP tools, WAL audit log
- Agents dashboard shows real data from live endpoints
- Dashboard HTML loaded via `include_str!`
- Web UIs served statically under `/web/`

### 🟡 Partial / Broken
- **Dev Sandbox**: Endpoints use `/api/*` prefix — 404 locally (Access-gated proxy in production). Need to detect local vs proxy mode
- **RAG queries**: Return synthetic mock data, not real results
- **AI Chat**: Falls back to hardcoded model list — Ollama not running

### 🔴 Not Working
- **RAG ingest**: No Python orchestrator running → uploads fail
- **Ollama connection**: Model bridge gets connection refused
- **Orchestrator proxy**: Python service not started
- **ChromaDB**: Vector database not running
- **VictoriaMetrics**: Metrics DB not running
- **Cloudflare Access + Tunnel**: Can't test locally

## Quick Fix Priority

```mermaid
gantt
    title Fix Plan
    dateFormat  YYYY-MM-DD
    axisFormat  %m-%d
    
    section Immediate
    Fix Dev Sandbox /api/ prefix    :done, a1, 2026-05-30, 1d
    Start Ollama + model            :a2, 2026-05-30, 1d
    
    section Short-term
    Wire AI Chat to real Ollama      :a3, after a2, 1d
    Start Python orchestrator        :a4, after a2, 1d
    
    section Medium-term
    Test RAG upload→query end-to-end  :a5, after a4, 1d
    Full native bring-up (systemd + cloudflared) :a6, after a5, 1d
    
    section Ship
    Commit, push, deploy             :a7, after a6, 1d
```
