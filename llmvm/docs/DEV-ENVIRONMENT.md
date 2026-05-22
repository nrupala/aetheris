# DEV Environment — Documentation

## Overview
A full VS Code development environment accessible via browser at **https://dev.nrupalakolkar.com**

Built on `code-server` (VS Code 1.95+) running inside a Docker container, exposed via Cloudflare Tunnel with zero inbound ports.

---

## Capabilities

### Code Editor (Full VS Code)
| Feature | Status | Details |
|---|---|---|
| **Editor** | ✅ Full VS Code | Syntax highlighting, IntelliSense, multi-cursor, split panes, minimap |
| **Extensions** | ✅ Install any | Marketplace access — install any extension from within VS Code |
| **Git** | ✅ Built-in | Commit, push, pull, branch management, diff view — all via UI |
| **Search** | ✅ Full-text | `Ctrl+Shift+F` with ripgrep backend, regex support, file filters |
| **Terminal** | ✅ Bash shell | Full Linux terminal inside container |
| **File Explorer** | ✅ Tree view | Create, rename, delete, move files |
| **Settings Sync** | ✅ Per-user | Settings stored in `/config/data/User/` (persistent volume) |

### Installed Toolchain
| Tool | Version | Usage |
|---|---|---|
| **Rust** | 1.95.0 | `rustc`, `cargo`, `clippy`, `rustfmt` |
| **Python** | 3.12.3 | `python3`, `pip3`, `venv` |
| **opencode** | 0.0.55 | AI coding assistant — run `opencode` in terminal |
| **ripgrep** | 14.1.0 | Fast text search — `rg "pattern"` |
| **fzf** | 0.44.1 | Fuzzy finder — pipe into `fzf` |
| **curl** | System | HTTP requests, API testing |
| **git** | System | Version control |
| **build-essential** | GCC 13 | C/C++ compilation |

### Python Packages (Pre-installed)
```
requests    HTTP client library
numpy       Numerical computing
tiktoken    OpenAI tokenizer
fastapi     Web framework
uvicorn     ASGI server
httpx       Async HTTP client
pytest      Testing framework
```

### Workspace Files
Mounted from host into `/config/workspace/`:
- `rag_core/` — RAG service source code (FastAPI)
- `rag_cli.py` — RAG command-line client
- `requirements.txt` — Python dependencies

---

## API Access (from inside terminal)
```bash
# Call LMStudio (host machine)
curl http://host.docker.internal:1234/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"phi-4-reasoning-plus","messages":[{"role":"user","content":"Hello"}]}'

# Call RAG service
curl http://localhost:8080/health
curl http://localhost:8080/ingest -F "file=@document.pdf"
curl http://localhost:8080/query -d '{"question":"What is the revenue?"}'
```

---

## Limitations

### ❌ No Docker-in-Docker
Cannot run `docker` commands inside the dev container. The container has no Docker socket mounted.

### ❌ No Root by Default
The `abc` user (non-root) is the default. Some operations may require `sudo` or switching to root:
```bash
# Switch to root (temporary)
docker exec -u root llmvm_dev <command>
```

### ❌ No Systemd/Services
Cannot start background services with `systemctl`. Use `&` for backgrounding:
```bash
python3 -m uvicorn app:app --host 0.0.0.0 --port 8001 &
```

### ❌ No GUI Applications
Browser-based VS Code — no X11/Wayland. Cannot run graphical apps.

### ❌ Limited Disk Space
Container filesystem is ephemeral except:
- `/config/` — Persistent (Docker volume)
- `/config/workspace/` — Mounted from host

### ❌ Port Binding
Can only bind to ports inside the container. External access requires Cloudflare Tunnel configuration.

### ❌ Host Filesystem Access
Cannot directly access `C:\Users\...` on Windows. Only mounted volumes are visible.

---

## Password
`6WqwZ&4k1rr9#ety`

## Architecture
```
Browser → Cloudflare (TLS) → Tunnel Container → code-server (port 8443)
```
Zero inbound ports on your machine. All traffic flows outbound through Cloudflare.

## How to Restart
```powershell
# On your Windows host:
docker compose up -d llmvm-dev
```

## Adding More Tools
```bash
# Inside the dev terminal (as root):
sudo apt-get install <package>

# Or via docker exec from host:
docker exec -u root llmvm_dev apt-get install <package>
```
