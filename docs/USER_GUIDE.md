# User Guide — Aetheris RAG

## Getting Started

Aetheris RAG is your personal AI that answers questions based on **your documents**. Upload files, ask questions, get answers with citations.

---

## Quick Start (3 Steps)

### Step 1: Upload a Document

**Via Web UI**:
1. Open the RAG UI at `https://rag.nrupalakolkar.com`
2. Click "Upload File"
3. Select a text file (`.txt`, `.md`, `.py`, `.rs`, etc.)
4. Click "Upload"
5. Wait for processing to complete (shows progress)

**Via API**:
```bash
curl -X POST "https://rag.nrupalakolkar.com/ingest/file" \
  -F "file=@docs/guide.md" \
  -b "aetheris_auth=your_token"
```

### Step 2: Ask a Question

**Via Web UI**:
1. Type your question in the search box
2. Press Enter or click "Search"
3. Read the answer with source citations

**Via API**:
```bash
curl -X POST "https://rag.nrupalakolkar.com/query" \
  -H "Content-Type: application/json" \
  -d '{"query": "How do I set up WireGuard?"}' \
  -b "aetheris_auth=your_token"
```

### Step 3: Check Results

The answer includes:
- **Answer**: The AI's response
- **Sources**: Which documents were used, with relevance scores
- **Response Time**: How long it took

---

## Uploading Documents

### Supported File Types

| Type | Extensions |
|------|-----------|
| Text | `.txt`, `.md` |
| Code | `.py`, `.rs`, `.js`, `.ts` |
| Config | `.json`, `.yaml`, `.yml`, `.toml`, `.cfg`, `.ini` |
| Web | `.html`, `.css` |

### File Size Limit

**Maximum**: 50 MB per file

If your file is larger, split it into smaller files first.

### Upload Methods

#### Method 1: Web UI (Easiest)
- Click "Upload File" button
- Drag and drop or browse
- Watch progress indicator

#### Method 2: API with Wait
```bash
# Blocks until upload is processed
curl -X POST "https://rag.nrupalakolkar.com/ingest/file?wait=true" \
  -F "file=@docs/guide.md" \
  -b "aetheris_auth=your_token"
```

Response:
```json
{
  "status": "completed",
  "chunks_created": 15,
  "time_seconds": 3.2,
  "chunks_per_second": 4.7
}
```

#### Method 3: API Async (for large files)
```bash
# Returns immediately, poll for status
curl -X POST "https://rag.nrupalakolkar.com/ingest/file" \
  -F "file=@docs/large-manual.md" \
  -b "aetheris_auth=your_token"
```

Response:
```json
{
  "status": "queued",
  "job_id": "abc123-def456-ghi789",
  "filename": "large-manual.md",
  "poll_url": "/jobs/abc123-def456-ghi789"
}
```

Poll for status:
```bash
curl "https://rag.nrupalakolkar.com/jobs/abc123-def456-ghi789" \
  -b "aetheris_auth=your_token"
```

---

## Asking Questions

### Basic Query

```json
POST /query
{
  "query": "What is WireGuard?"
}
```

### Advanced Query Options

| Option | Default | Description |
|--------|---------|-------------|
| `use_rag` | `true` | Set `false` for pure LLM (no document search) |
| `top_k` | `3` | Number of document chunks to search |
| `threshold` | `0.7` | Minimum relevance score (0.0-1.0) |
| `include_history` | `true` | Set `false` to ignore previous questions |

**Example: Strict search**
```json
{
  "query": "What port does WireGuard use?",
  "top_k": 5,
  "threshold": 0.8
}
```

**Example: Pure LLM (no documents)**
```json
{
  "query": "Explain quantum computing",
  "use_rag": false
}
```

---

## Managing Documents

### List Indexed Sources

```bash
curl "https://rag.nrupalakolkar.com/sources" \
  -b "aetheris_auth=your_token"
```

Response:
```json
[
  {"source": "docs/wireguard.txt", "chunks": 15, "last_seen": "2026-05-03"},
  {"source": "docs/networking.md", "chunks": 23, "last_seen": "2026-05-02"}
]
```

### Delete a Source

```bash
curl -X DELETE "https://rag.nrupalakolkar.com/sources/docs/wireguard.txt" \
  -b "aetheris_auth=your_token"
```

### View Statistics

```bash
curl "https://rag.nrupalakolkar.com/stats" \
  -b "aetheris_auth=your_token"
```

Response:
```json
{
  "total_chunks": 156,
  "total_sources": 8,
  "total_tokens": 45230,
  "embedding_dimension": 768,
  "db_size_mb": 12.4
}
```

### Reset Everything

```bash
curl -X POST "https://rag.nrupalakolkar.com/reset" \
  -b "aetheris_auth=your_token"
```

⚠️ **Warning**: This deletes all indexed documents and vectors. Permanent.

---

## Troubleshooting

### "503 Service Unavailable"

**Cause**: The AI engine is temporarily down or the system is under heavy load.

**What to do**:
1. Wait 30 seconds and try again
2. The system automatically recovers (circuit breaker resets)
3. If persistent, check system status

### "File too large"

**Cause**: File exceeds 50 MB limit.

**What to do**:
- Split the file into smaller chunks
- Remove unnecessary content (images, binary data)

### "Empty file"

**Cause**: Uploaded file has no text content.

**What to do**:
- Ensure the file contains readable text
- Binary files (PDF, images) are not supported

### No relevant results

**Possible causes**:
1. Document not uploaded yet — check `/sources`
2. Question too specific — try broader terms
3. Threshold too high — lower `threshold` to 0.5

### Slow responses

**Possible causes**:
1. First query after idle (model loading) — normal, will be faster next time
2. Large context (many chunks) — reduce `top_k`
3. Queue wait (other queries in flight) — wait or retry

---

## Best Practices

### For Better Answers

1. **Upload relevant documents** — The AI can only answer based on what you've uploaded
2. **Use specific questions** — "How do I configure WireGuard on Linux?" beats "Tell me about networking"
3. **Adjust threshold** — Higher threshold = more relevant but fewer results; lower = more results but may include noise
4. **Enable history** — Keep `include_history: true` for conversational context

### For Faster Uploads

1. **Keep files under 10 MB** — Smaller files process faster
2. **Upload during off-peak** — Less queue wait
3. **Use async mode** — `wait=false` lets you upload without blocking

### For Better Performance

1. **Start with a few documents** — Index 2-3 files first, test queries, then add more
2. **Use text formats** — `.txt` and `.md` process fastest
3. **Clean your data** — Remove boilerplate, headers, footers before uploading

---

## Web UI Guide

### Navigation

```
┌─────────────────────────────────────────────┐
│  Aetheris RAG                    [⚙️] [📊] │
├─────────────────────────────────────────────┤
│                                             │
│  🔍 [Search box                  ] [Search] │
│                                             │
│  ────────────────────────────────────────   │
│                                             │
│  💬 Answer appears here                     │
│                                             │
│  📎 Sources:                                │
│  • docs/wireguard.txt (score: 0.89)        │
│  • docs/networking.md (score: 0.85)        │
│                                             │
│  ────────────────────────────────────────   │
│                                             │
│  📁 [Upload File]                           │
│                                             │
└─────────────────────────────────────────────┘
```

### UI Elements

| Element | Function |
|---------|----------|
| Search box | Type your question |
| [Search] button | Execute query |
| [⚙️] Settings | Adjust threshold, top_k, history |
| [📊] Stats | View index statistics |
| Answer area | AI response with formatting |
| Sources list | Documents used, with relevance scores |
| [Upload File] | Upload new documents |

---

## Frequently Asked Questions

**Q: What types of files can I upload?**  
A: Text files (`.txt`, `.md`), code files (`.py`, `.rs`, `.js`, `.ts`), and config files (`.json`, `.yaml`, `.toml`).

**Q: Can I upload PDFs?**  
A: Not directly. Convert PDFs to text first, then upload the `.txt` file.

**Q: How many documents can I index?**  
A: No hard limit. Practical limit depends on available storage and RAM.

**Q: Does the AI learn from my documents?**  
A: Yes! The Knowledge Graph extracts entities and relationships from your documents, making future answers more personalized.

**Q: Can I ask questions without uploading documents?**  
A: Yes, set `use_rag: false` for pure LLM answers (no document search).

**Q: Are my documents stored permanently?**  
A: Uploaded files are stored permanently in `/persisted/storage/`. You can delete them via the `/sources/{path}` endpoint.

**Q: Is my data private?**  
A: Yes. All data stays on your server. No external services are used for storage or processing.
