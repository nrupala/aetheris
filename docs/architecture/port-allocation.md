# Port Allocation & Service Discovery

Aetheris uses a dynamic port allocation system to avoid conflicts and enable
zero-trust service discovery at runtime.

## How It Works

```
port_allocator.sh ──► config/port_registry.json ──► aetheris-core reads at boot
                              │
                    export env vars ──► Docker Compose
                              │
                    aetheris-core serves GET /discovery
```

Every service port is configurable via environment variables with sensible defaults.
The port allocator scans for available ports and writes:

1. **Shell exports** for `eval "$(bash scripts/port_allocator.sh)"`
2. **JSON registry** at `config/port_registry.json`

## Port Variables

| Variable | Default | Service |
|----------|---------|---------|
| `AETHERIS_CORE_PORT` | 8080 | Rust core API |
| `OPA_GATEWAY_PORT` | 8181 | OPA policy engine |
| `VICTORIA_METRICS_PORT` | 8428 | Metrics database |
| `NGINX_HTTP_PORT` | 80/9080 | Web proxy (HTTP) |
| `NGINX_HTTPS_PORT` | 443/9443 | Web proxy (TLS) |
| `RAG_SERVICE_PORT` | 8081 | RAG pipeline |
| `LLMVM_DEV_PORT` | 8443 | Code-server sandbox |
| `LLMVM_ORCHESTRATOR_PORT` | 9090 | Multi-agent orchestrator |
| `WIREGUARD_PORT` | 51820 | Mesh VPN |
| `OLLAMA_PORT` | 11434 | AI engine |
| `CHROMA_PORT` | 8000 | Vector database |

## Usage

```bash
# Generate port assignments
eval "$(bash scripts/port_allocator.sh)"

# Start stack with allocated ports
docker compose up -d

# Query the registry from any service
curl http://localhost:8080/discovery
```

## CI Integration

In CI pipelines, `port_allocator.sh` runs before Docker Compose and
exports ports to `$GITHUB_ENV` so all downstream steps use the same
allocations. UAT scripts read `AETHERIS_CORE_PORT`, `OPA_GATEWAY_PORT`,
etc. rather than hardcoding.

## Security

The `/discovery` endpoint is served by the OPA-gated core. Only
authenticated services within the mesh can enumerate available ports.
This prevents port scanning from untrusted sources.
