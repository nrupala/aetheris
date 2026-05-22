# AI Environment — Documentation

## Overview
Direct access to your local LMStudio inference engine via **https://ai.nrupalakolkar.com**

This is NOT a separate service — it's a Cloudflare Tunnel route directly to your host machine's LMStudio API on port 1234.

---

## Capabilities

### OpenAI-Compatible API
LMStudio exposes a drop-in replacement for the OpenAI API:
- `/v1/chat/completions` — Chat completions
- `/v1/completions` — Legacy completions
- `/v1/embeddings` — Text embeddings
- `/v1/models` — List available models

### 21 Installed Models
| Model | Type | Size | Use Case |
|---|---|---|---|
| `phi-4-reasoning-plus` | Chat | ~9B | Reasoning, logic, math |
| `qwen2.5-coder-14b` | Chat | 14B | Code generation, debugging |
| `qwen2.5-coder-1.5b` | Chat | 1.5B | Fast code tasks |
| `nomic-embed-text` | Embedding | 137M | Semantic search, RAG |
| `deepseek-r1-distill-qwen-14b` | Chat | 14B | Reasoning with CoT |
| `llama-3.2-3b` | Chat | 3B | Fast general tasks |
| `mistral-7b-instruct` | Chat | 7B | Instruction following |
| `gemma-2-9b` | Chat | 9B | General purpose |
| *(+ 13 more loaded models)* | | | |

### Usage Examples
```bash
# Chat completion
curl -X POST https://ai.nrupalakolkar.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "phi-4-reasoning-plus",
    "messages": [{"role": "user", "content": "Explain quantum computing"}],
    "max_tokens": 500,
    "temperature": 0.7
  }'

# List all models
curl https://ai.nrupalakolkar.com/v1/models

# Generate embeddings
curl -X POST https://ai.nrupalakolkar.com/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{
    "model": "nomic-embed-text",
    "input": "Hello world"
  }'
```

### Python Integration
```python
import requests

response = requests.post(
    "https://ai.nrupalakolkar.com/v1/chat/completions",
    json={
        "model": "qwen2.5-coder-14b",
        "messages": [{"role": "user", "content": "Write a quicksort in Python"}],
        "max_tokens": 300
    }
)
print(response.json()["choices"][0]["message"]["content"])
```

### OpenAI SDK Compatible
```python
from openai import OpenAI

client = OpenAI(
    base_url="https://ai.nrupalakolkar.com/v1",
    api_key="lm-studio"  # Any value works
)

response = client.chat.completions.create(
    model="phi-4-reasoning-plus",
    messages=[{"role": "user", "content": "What is 2+2?"}]
)
print(response.choices[0].message.content)
```

---

## Limitations

### ⚠️ No Authentication (Currently)
The AI endpoint is **open** — anyone who knows the URL can query it. A password (`efqbqFUzj841_r-5`) is configured but not yet enforced at the tunnel level.

### ⚠️ Requires LMStudio Running
If LMStudio is closed on your host machine, this endpoint returns errors. No standalone inference.

### ⚠️ GPU-Dependent Performance
Inference speed depends on your local GPU. Cloudflare Tunnel adds ~50-100ms latency.

### ⚠️ Rate Limits
No built-in rate limiting. Heavy concurrent usage may queue requests.

### ❌ No Model Management
Cannot upload, download, or manage models through this endpoint. Use LMStudio desktop UI.

### ❌ No Training/Fine-tuning
Inference only. No LoRA, no fine-tuning, no training endpoints.

### ❌ No Streaming via Tunnel
Streaming responses (`stream: true`) may be unstable through Cloudflare Tunnel.

---

## Password
`efqbqFUzj841_r-5` (configured, not yet enforced at tunnel level)

## Architecture
```
Browser → Cloudflare (TLS) → Tunnel → host.docker.internal:1234 (LMStudio)
```
Direct pass-through. No middleware, no modification of responses.

## Dependencies
- LMStudio running on host at `localhost:1234`
- Cloudflare Tunnel container (`llmvm_tunnel`)
- DNS: `ai.nrupalakolkar.com` → Cloudflare CNAME
