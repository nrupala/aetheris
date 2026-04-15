# AETHERIS - API SPECIFICATION
## v1.0

---

## BASE URLs

| Environment | URL |
|-------------|-----|
| Local (container) | http://aetheris-core:8080 |
| Mesh (WireGuard) | http://10.0.0.1:8080 |
| External | https://your-domain.com:51820 (WireGuard only) |

---

## CORE API

### GET /
**Description:** Zero-JS Dashboard HTML
```yaml
Response:
  200:
    Content-Type: text/html
    Body: (HTML page - no JavaScript)
```

---

### POST /upload
**Description:** Upload file to encrypted vault
```yaml
Request:
  Method: POST
  Content-Type: multipart/form-data
  Headers:
    X-Aetheris-Peer: <peer_id>
    Authorization: Bearer <jwt_token>
  Body:
    file: (binary file data)
Response:
  200:
    {
      "status": "uploaded",
      "filename": "document.pdf",
      "size": 102400,
      "indexed": true
    }
  403:
    {
      "error": "forbidden",
      "reason": "invalid_token"
    }
```

---

### GET /download/:filename
**Description:** Download file from vault
```yaml
Request:
  Method: GET
  Path: /download/:filename
  Headers:
    X-Aetheris-Peer: <peer_id>
    Authorization: Bearer <jwt_token>
Response:
  200:
    Content-Type: application/octet-stream
    Content-Disposition: attachment; filename="document.pdf"
    Body: (binary file data)
  403:
    {"error": "forbidden"}
  404:
    {"error": "not_found"}
```

---

### GET /search
**Description:** Semantic search across vault
```yaml
Request:
  Method: GET
  Query Parameters:
    q: (string) Semantic search query
    limit: (int, optional) Max results (default: 10)
  Headers:
    X-Aetheris-Peer: <peer_id>
    Authorization: Bearer <jwt_token>
Response:
  200:
    {
      "query": "budget 2024 tax",
      "results": [
        {
          "filename": "tax_returns.pdf",
          "score": 0.95,
          "excerpt": "...federal tax returns for 2024...",
          "path": "/vault/documents/tax_returns.pdf"
        }
      ],
      "total": 1
    }
  403:
    {"error": "forbidden"}
```

---

### GET /status
**Description:** System health status
```yaml
Request:
  Method: GET
  Headers:
    X-Aetheris-Peer: <peer_id>
Response:
  200:
    {
      "version": "1.0.0",
      "uptime": 3600,
      "components": {
        "vault": {
          "status": "encrypted_mounted",
          "usage_bytes": 10737418240,
          "encryption": "AES-256-GCM"
        },
        "mesh": {
          "status": "active",
          "peers": 3,
          "subnet": "10.0.0.0/24"
        },
        "ai": {
          "status": "ready",
          "model": "mistral",
          "embedding_model": "nomic-embed-text"
        },
        "vector_db": {
          "status": "connected",
          "collections": 1,
          "indexed_files": 150
        }
      },
      "security": {
        "auto_ban": "active",
        "banned_peers": 0,
        "ghost_shell": "armed"
      }
    }
```

---

### GET /metrics
**Description:** Prometheus metrics endpoint
```yaml
Request:
  Method: GET
Response:
  200:
    Content-Type: text/plain; version=0.0.4
    Body: |
      # HELP aetheris_vault_usage_bytes Current encrypted storage usage
      # TYPE aetheris_vault_usage_bytes gauge
      aetheris_vault_usage_bytes 10737418240
      
      # HELP aetheris_security_violations_total Total blocked peer requests
      # TYPE aetheris_security_violations_total counter
      aetheris_security_violations_total 0
      
      # HELP aetheris_files_indexed_total Total files indexed
      # TYPE aetheris_files_indexed_total counter
      aetheris_files_indexed_total 150
      
      # HELP aetheris_search_queries_total Total semantic queries
      # TYPE aetheris_search_queries_total counter
      aetheris_search_queries_total 42
```

---

## SECURITY API (OPA)

### POST /v1/data/aetheris/authz/allow
**Description:** Authorization decision
```yaml
Request:
  Method: POST
  Content-Type: application/json
  Body:
    {
      "input": {
        "token": "eyJhbGci...",
        "user_role": "admin",
        "method": "GET",
        "action": "read_vault",
        "path": "/download/report.pdf",
        "peer_id": "wg-peer-01",
        "file_metadata": {
          "sensitivity": "normal"
        }
      }
    }
Response:
  200:
    {"result": true}
  200:
    {"result": false}
```

---

### GET /health
**Description:** OPA health check
```yaml
Response:
  200:
    {"status": "ok"}
```

---

## AI API (Ollama)

### POST /api/generate
**Description:** LLM text generation
```yaml
Request:
  Method: POST
  Content-Type: application/json
  Body:
    {
      "model": "mistral",
      "prompt": "What is Aetheris?",
      "stream": false,
      "options": {
        "temperature": 0.7,
        "max_tokens": 500
      }
    }
Response:
  200:
    {
      "model": "mistral",
      "response": "Aetheris is a sovereign AI-native personal cloud...",
      "done": true,
      "context": [1, 2, 3, ...],
      "total_duration": 5000000000,
      "load_duration": 2000000000,
      "prompt_eval_count": 10,
      "eval_count": 150
    }
```

---

### POST /api/embeddings
**Description:** Generate text embeddings
```yaml
Request:
  Method: POST
  Content-Type: application/json
  Body:
    {
      "model": "nomic-embed-text",
      "prompt": "Document content to embed"
    }
Response:
  200:
    {
      "model": "nomic-embed-text",
      "embedding": [
        0.0123, -0.0456, 0.0789, ...
      ],
      "total_duration": 1000000000
    }
```

---

### GET /api/tags
**Description:** List available models
```yaml
Response:
  200:
    {
      "models": [
        {
          "name": "mistral:latest",
          "size": 4109780147,
          "digest": "sha256:...",
          "modified_at": "2024-01-15T10:30:00Z"
        },
        {
          "name": "nomic-embed-text:latest",
          "size": 274510974,
          "digest": "sha256:...",
          "modified_at": "2024-01-10T08:00:00Z"
        }
      ]
    }
```

---

## VECTOR API (ChromaDB)

### POST /api/v1/collections/aetheris/add
**Description:** Add document to vector store
```yaml
Request:
  Method: POST
  Content-Type: application/json
  Body:
    {
      "ids": ["doc-001"],
      "embeddings": [[0.1, 0.2, ...]],
      "documents": ["Document content here..."],
      "metadatas": [
        {
          "filename": "report.pdf",
          "path": "/vault/documents/report.pdf",
          "size": 102400,
          "indexed_at": "2024-01-15T10:30:00Z"
        }
      ]
    }
Response:
  200:
    {"ids": ["doc-001"], "success": true}
```

---

### POST /api/v1/collections/aetheris/query
**Description:** Query vector store
```yaml
Request:
  Method: POST
  Content-Type: application/json
  Body:
    {
      "query_embeddings": [[0.1, 0.2, ...]],
      "n_results": 5,
      "where": {"filename": {"$contains": ".pdf"}}
    }
Response:
  200:
    {
      "ids": [["doc-001", "doc-002"]],
      "distances": [[0.05, 0.12]],
      "documents": [["Content of result 1...", "Content of result 2..."]],
      "metadatas": [[{"filename": "report.pdf", ...}, {...}]]
    }
```

---

### GET /api/v1/heartbeat
**Description:** ChromaDB health check
```yaml
Response:
  200:
    {"nanosecond heartbeat": 1705312200000000000}
```

---

## METRICS API (VictoriaMetrics)

### GET /api/v1/query
**Description:** Query metrics
```yaml
Request:
  Method: GET
  Query Parameters:
    query: (string) PromQL query
    time: (optional) Unix timestamp
Response:
  200:
    {
      "status": "success",
      "data": {
        "resultType": "vector",
        "result": [
          {
            "metric": {"__name__": "up", "job": "aetheris"},
            "value": [1705312200, "1"]
          }
        ]
      }
    }
```

---

### GET /health
**Description:** VictoriaMetrics health check
```yaml
Response:
  200:
    {"status": "ok"}
```

---

## ERROR CODES

| Code | Meaning | Resolution |
|------|---------|------------|
| 200 | Success | - |
| 400 | Bad Request | Check request format |
| 401 | Unauthorized | Validate JWT token |
| 403 | Forbidden | Check OPA policy |
| 404 | Not Found | Check filename/path |
| 429 | Rate Limited | Wait and retry |
| 500 | Server Error | Check component logs |
| 503 | Service Unavailable | Check dependencies |

---

## RATE LIMITS

| Endpoint | Limit |
|----------|-------|
| /upload | 10 MB/s |
| /download | 100 MB/s |
| /search | 60 requests/minute |
| /api/generate | 10 requests/minute |

---

**API VERSION:** 1.0
**STATUS:** APPROVED
**DATE:** 2026-04-15
