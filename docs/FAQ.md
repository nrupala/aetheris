# Aetheris — Frequently Asked Questions

## General

### What is Aetheris?
Aetheris is a Sovereign AI-Native Personal Cloud — a self-hosted platform that combines local LLM inference (via Ollama), document Q&A (via RAG), multi-agent orchestration, and secure file management in a zero-trust architecture.

### What does Aetheris cost to run?
The core services run at $0/month — everything is local. The only potential cost is your Cloudflare Tunnel (free tier available) and your electricity/hosting.

### What hardware do I need?
- **Minimum:** 4 CPU cores, 8GB RAM, 20GB storage
- **Recommended:** 8 CPU cores, 16GB RAM, 100GB+ storage (SSD)
- **For LLM:** qwen2.5:14b requires ~8GB RAM just for the model

## Access & Authentication

### I get "401 Unauthorized" — what's the password?
All users share the same password: `BCjfTYIIjMASFGVM`. The usernames are:
- `ai_user` for AI chat
- `rag_user` for RAG document Q&A
- `dev_user` for Agent Dashboard and Dev Sandbox

### The password doesn't work. What should I do?
The password is configured via Nginx `htpasswd`. Contact your admin to verify the environment variable `NGINX_BASIC_AUTH` in Docker Compose.

### Can I change the password?
Yes. Generate a new htpasswd hash and update the `NGINX_BASIC_AUTH` environment variable, then restart nginx.

## AI

### What model is Aetheris using?
The production server runs a single model: `qwen2.5:14b` — a 14 billion parameter model from the Qwen 2.5 family.

### Can I use a different model?
If Ollama has the model loaded, you can select it from the dropdown in the AI Chat UI. To pull a new model:
```bash
docker compose exec ollama ollama pull <model-name>
```

### Why is the AI slow?
Response time depends on:
- Model size (qwen2.5:14b is 14B parameters)
- Available RAM/VRAM
- Concurrent requests
- Prompt length (longer prompts = slower generation)

Typical response time: 5-30 seconds for most queries.

### Is my chat data private?
Yes. All inference happens locally on Ollama. No data leaves your machine. The connection to the browser is encrypted via Cloudflare Tunnel.

## RAG

### What file types are supported?
PDF, TXT, MD, HTML, JSON, CSV. Maximum file size: 50MB.

### How long does indexing take?
For a typical 100-page PDF: 10-30 seconds. Depends on file size, chunking, embedding time, and system load.

### How many documents can I upload?
Limited only by available storage. Each document is chunked into ~500-token pieces and stored as embeddings in SQLite + ChromaDB.

### Why did I get "No relevant results"?
Possible reasons:
- No documents indexed yet (upload documents first)
- Query doesn't match any content (try different wording)
- Similarity threshold too high (try lowering from 0.7 to 0.5)
- Top K too low (try increasing from 5 to 10)

### What is Reasoning Mode?
When enabled, the RAG pipeline uses iterative self-verification with temperature annealing (0.8→0.5→0.1) to improve answer quality for complex, multi-part questions. It continues iterating until confidence exceeds the threshold or max iterations are reached.

## Agents

### What does the Agent Orchestrator do?
It runs multi-agent workflows with a pipeline: **Planner → Researcher → Coder → Reviewer**. The Planner decomposes tasks, the Researcher gathers information from RAG/KG, the Coder produces output, and the Reviewer validates quality.

### How do I run a workflow?
Go to the Agents dashboard (`https://agents.nrupalakolkar.com`), open the Workflow tab, describe your task, and click Run. Or use the API:
```bash
curl -u dev_user:BCjfTYIIjMASFGVM \
  https://agents.nrupalakolkar.com/api/workflow/run \
  -H "Content-Type: application/json" \
  -d '{"task": "Your task here"}'
```

### What are MCP Tools?
MCP (Model Context Protocol) tools are capabilities that agents can discover and invoke — including file operations, RAG queries, KG access, code evaluation, and system commands. All gated through OPA policy checks.

### What is A2A?
A2A (Agent-to-Agent) is a messaging protocol that enables direct communication between agents for coordination, handoff, and result sharing.

## Dev Sandbox

### What is the Dev Sandbox for?
The Dev Sandbox (`https://dev.nrupalakolkar.com`) provides a browser-based interface for:
- **API Console:** Execute any Aetheris API endpoint with method/body control
- **System Logs:** View the live WAL-backed audit trail
- **Config:** View runtime configuration from `/etc/aetheris/`
- **Metrics:** Service health dashboard with uptime and status monitoring

### How do I use the API Console?
1. Select an endpoint from the dropdown
2. Choose the HTTP method
3. For POST endpoints, enter a JSON body
4. Click Execute
5. See the response with status code, latency, and body

## Deployment

### How do I deploy Aetheris?
```bash
git clone <repo>
docker compose build
docker compose up -d
```
For detailed instructions, see the [Deployment Guide](guides/deployment.md).

### What ports need to be open?
Only the Cloudflare Tunnel needs outbound access. All service ports (8080, 11434, 9090) are internal to Docker.

### How do I update Aetheris?
```bash
git pull
docker compose build core
docker compose up -d --force-recreate core
```

### How do I back up my data?
```bash
# RAG data
docker cp <rag-container>:/app/data ./backups/rag/

# WAL (audit log)
cp -r /path/to/vault/wal/ ./backups/wal/
```

## Troubleshooting

### I get "502 Bad Gateway"
Likely the backend service is down. Check:
```bash
docker compose ps
docker compose logs core
```

### The health check fails
```bash
curl -u dev_user:BCjfTYIIjMASFGVM https://dev.nrupalakolkar.com/api/health
```
If it returns an error, check Docker service status.

### Cloudflare Tunnel is down
```bash
cloudflared tunnel list
cloudflared tunnel logs <tunnel-id>
cloudflared tunnel restart <tunnel-id>
```

### I found a bug
Open an issue at the repository or report it via the project's issue tracker.

### How do I reset everything?
```bash
docker compose down -v   # ⚠️ This deletes ALL data
docker compose build --no-cache
docker compose up -d
```
