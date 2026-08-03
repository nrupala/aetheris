# Aetheris — Frequently Asked Questions

## General

### What is Aetheris?
Aetheris is a Sovereign AI-Native Personal Cloud — a self-hosted platform that combines local LLM inference (via Ollama), document Q&A (via RAG), multi-agent orchestration, and secure file management in a zero-trust architecture.

### What does Aetheris cost to run?
The core services run at $0/month — everything is local. The only potential cost is your Cloudflare Tunnel (free tier available) and your electricity/hosting.

### What hardware do I need?
- **Minimum:** 4 CPU cores, 8GB RAM, 20GB storage
- **Recommended:** 8 CPU cores, 16GB RAM, 100GB+ storage (SSD)
- **For LLM:** qwen2.5:7b requires ~5GB RAM just for the model

## Access & Authentication

### I get "401 Unauthorized" / "403 Forbidden" — how do I authenticate?
Access is gated by **Cloudflare Access** at the edge (no HTTP Basic Auth). Interactive users sign in with their identity; scripted/API callers send a Cloudflare Access **service token** as headers:
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" \
     -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
     https://core.nrupalakolkar.com/api/health
```
The token values live in your environment / secret store — never in this repo.

### Access is denied. What should I do?
Confirm your identity (or service token) is allowed by the Cloudflare Access policy for the application. Manage policies and service tokens from the Cloudflare Zero Trust dashboard.

### Can I change who has access?
Yes — edit the Cloudflare Access application's policy (add/remove emails, rotate the service token). No `htpasswd` and no service restart required.

## AI

### What model is Aetheris using?
The production server runs a single model: `qwen2.5:7b` — a 7 billion parameter model from the Qwen 2.5 family.

### Can I use a different model?
If Ollama has the model loaded, you can select it from the dropdown in the AI Chat UI. To pull a new model:
```bash
ollama pull <model-name>
```

### Why is the AI slow?
Response time depends on:
- Model size (qwen2.5:7b is 7B parameters)
- Available RAM/VRAM
- Concurrent requests
- Prompt length (longer prompts = slower generation)

Typical response time: 5-30 seconds for most queries.

### Is my chat data private?
Yes. All inference happens locally on Ollama. No data leaves your machine. The connection to the browser is encrypted via Cloudflare Tunnel.

## RAG

### What file types are supported?
PDF, TXT, MD, HTML, JSON, CSV/TSV, YAML, XML, and common code files (.rs, .py, .js, .ts, .go, .c/.cpp, etc.). Maximum upload size: 50MB.

### How long does indexing take?
For a typical 100-page PDF: 10-30 seconds. Depends on file size, chunking, embedding time, and system load.

### How many documents can I upload?
Limited only by available storage. Each document is chunked into ~512-token pieces and stored in the SQLite vector store (`vectors.db`) as normalized 768-dim embeddings.

### Why did I get a weak or empty answer?
Possible reasons:
- No documents indexed yet (upload documents first)
- Query doesn't match any indexed content (try different wording)
- Top K too low (try increasing from 5 to 10)
- The first query pays a model cold-load cost on CPU — retry once the model is warm

### What is Reasoning Mode?
When enabled, the model is asked to explain its thought process and returns a `reasoning` field alongside the answer. There is no iterative loop or temperature annealing — generation is a single pass.

### What happened to the reranker?
Reranking (`bge-reranker-v2-m3`) is disabled by default because the deployed Ollama build does not expose `/api/rerank`. Queries fall back to vector-search order. Enable it only after upgrading Ollama to a build with rerank support.

## Agents

### What does the Agent Orchestrator do?
It runs multi-agent workflows with a pipeline: **Planner → Researcher → Coder → Reviewer**. The Planner decomposes tasks, the Researcher gathers information from RAG/KG, the Coder produces output, and the Reviewer validates quality.

### How do I run a workflow?
Go to the Agents dashboard (`https://agents.nrupalakolkar.com`), open the Workflow tab, describe your task, and click Run. Or use the API:
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" \
     -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
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
sudo scripts/install-native.sh
```
The core runs as a native systemd service; cloudflared points at `127.0.0.1:8080` and Cloudflare Access gates the public hostname. For detailed instructions, see the [Native Deployment Guide](DEPLOY_NATIVE.md).

### What ports need to be open?
None inbound. Only the Cloudflare Tunnel needs outbound access. All service ports (8080, 11434, 9090) are bound to loopback (`127.0.0.1`) on the host.

### How do I update Aetheris?
```bash
git pull
sudo scripts/install-native.sh   # rebuilds the binary and restarts the service
```

### How do I back up my data?
```bash
# RAG data + WAL (audit log) live under the vault directory
cp -r /data/vault/ ./backups/vault/
```

## Troubleshooting

### I get "502 Bad Gateway"
Likely the core service is down. Check:
```bash
systemctl status aetheris-core
journalctl -u aetheris-core -n 100
```

### The health check fails
```bash
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" https://dev.nrupalakolkar.com/api/health
```
If it returns an error, check the `aetheris-core` service status.

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
sudo systemctl restart aetheris-core     # restart only
sudo scripts/install-native.sh           # rebuild + reinstall (keeps /etc/aetheris/core.env)
```
