# RAG Service — Documentation

## Overview
Retrieval-Augmented Generation API at **https://rag.nrupalakolkar.com**

RAG lets you ask questions about your documents and get answers grounded in your actual content — not the model's training data.

---

## How It Works

```
1. INGEST   → Documents are split into chunks and stored in SQLite + vector index
2. SEARCH   → Query is embedded (nomic-embed-text) and matched against indexed chunks
3. GENERATE → Top-k relevant chunks are sent to LLM as context for the answer
```

**Why RAG instead of raw LLM?**
- Raw LLM: "What was Q3 revenue?" → Makes up an answer from training data
- RAG: Embeds your question → finds the financial report in your docs → LLM answers from that specific text → Grounded, accurate response

---

## API Endpoints

### Health Check
```bash
curl https://rag.nrupalakolkar.com/health
# Response: {"status":"ok","service":"aetheris-rag"}
```

### Ingest Documents
```bash
curl -X POST https://rag.nrupalakolkar.com/ingest \
  -F "file=@document.pdf"

# Response:
# {"status":"success","chunks":24,"source":"document.pdf"}
```

**Supported formats:** `.txt`, `.md`, `.pdf`, `.html`, `.json`, `.csv`

### Query Documents
```bash
curl -X POST https://rag.nrupalakolkar.com/query \
  -H "Content-Type: application/json" \
  -d '{
    "question": "What was the total revenue in Q3?",
    "top_k": 5,
    "model": "phi-4-reasoning-plus"
  }'

# Response:
# {
#   "answer": "The total revenue in Q3 was $4.2M...",
#   "sources": ["financial-report-q3.pdf:page-12", "earnings-call-transcript.md:line-45"],
#   "context_chunks": [
#     {"text": "Q3 revenue reached $4.2M...", "source": "financial-report-q3.pdf", "score": 0.94}
#   ]
# }
```

### List Indexed Documents
```bash
curl https://rag.nrupalakolkar.com/documents
# Response:
# {"documents": [
#   {"name": "financial-report-q3.pdf", "chunks": 24, "indexed_at": "2026-05-02T..."}
# ]}
```

### Delete Document
```bash
curl -X DELETE https://rag.nrupalakolkar.com/documents/financial-report-q3.pdf
```

---

## Architecture

```
FastAPI (port 8080)
├── /ingest   → PDF/text extraction → chunking (500 tokens) → nomic embeddings → SQLite
├── /query    → Embed query → cosine similarity → top-k → prompt template → LMStudio
├── /documents → SQLite metadata queries
└── /health   → Service health check
```

### Technology Stack
| Component | Technology |
|---|---|
| **Framework** | FastAPI (Python 3.11) |
| **Vector Store** | SQLite + numpy cosine similarity |
| **Embeddings** | nomic-embed-text (via LMStudio) |
| **Generation** | LMStudio (host, port 1234) |
| **Tokenization** | tiktoken |
| **Container** | Docker (Python slim base) |

### Data Persistence
- Vector index: Docker volume `rag_data`
- Document metadata: Docker volume `rag_db`
- Both persist across container restarts

---

## Capabilities

### ✅ Semantic Search
Finds relevant content based on meaning, not just keywords.
- "How much did we earn?" → matches "revenue was $4.2M" even without keyword overlap

### ✅ Multi-Document Support
Ingest and query across many documents simultaneously.
- PDFs, text files, markdown, HTML, CSV all supported

### ✅ Source Attribution
Every answer includes the source documents and specific chunks used.
- No hallucinated citations — only real document references

### ✅ Configurable Parameters
- `top_k` — Number of context chunks (default: 5)
- `model` — Which LMStudio model to use
- Custom prompt templates supported

### ✅ Lightweight
No heavy vector databases (Milvus, Pinecone). Uses SQLite + numpy.
- ~50MB RAM usage
- Instant startup
- No external dependencies beyond LMStudio

---

## Limitations

### ⚠️ No Authentication (Currently)
The RAG endpoint is **open** — anyone who knows the URL can query it. A password (`H5epZhriylz+99+1`) is configured but not yet enforced.

### ⚠️ Requires LMStudio
Embeddings and generation both call LMStudio. If LMStudio is down, RAG returns errors.

### ⚠️ In-Memory Vector Search
Uses numpy cosine similarity, not a production vector DB. Fine for hundreds of documents, not millions.

### ⚠️ No Real-time Indexing
Documents are indexed at ingest time. No watching for file changes.

### ❌ No OCR
Cannot extract text from scanned PDFs or images.

### ❌ No Audio/Video
Text-only. No transcription or audio processing.

### ❌ No Multi-modal
Text embeddings only. No image or video understanding.

---

## Password
`H5epZhriylz+99+1` (configured, not yet enforced at tunnel level)

## Architecture
```
Browser → Cloudflare (TLS) → Tunnel → host.docker.internal:8080 (RAG FastAPI)
                                                         ↓
                                              host.docker.internal:1234 (LMStudio)
```

## Dependencies
- LMStudio running on host at `localhost:1234`
- Cloudflare Tunnel container (`llmvm_tunnel`)
- DNS: `rag.nrupalakolkar.com` → Cloudflare CNAME
- Docker volumes: `rag_data`, `rag_db`
