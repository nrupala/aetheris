# Aetheris Bare Metal Emulation

This directory contains Docker-based infrastructure that emulates a bare metal deployment of Aetheris for testing purposes.

## Overview

The emulation environment provides:

- **ZFS Emulation**: Simulates encrypted ZFS storage pools
- **WireGuard Mesh**: UDP 51820 VPN for encrypted networking
- **OPA Gateway**: Zero-Trust policy enforcement
- **Encrypted Vault**: HashiCorp Vault for secrets management
- **Ollama AI**: Local LLM for semantic search
- **VictoriaMetrics**: Time-series metrics
- **Ghost Shell**: Honeypot for threat detection

## Quick Start

```bash
# Build and run emulation environment
docker compose -f compose.emulation.yaml build
docker compose -f compose.emulation.yaml up -d

# Run UAT tests
bash tests/run_emulation_uat.sh

# View logs
docker compose -f compose.emulation.yaml logs -f

# Stop
docker compose -f compose.emulation.yaml down
```

## Network Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Aetheris Emulation                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  10.0.10.0/24 (aetheris_test_net)                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │
│  │WireGuard │ │   OPA    │ │  Vault   │ │  Ollama  │     │
│  │10.0.10.1 │ │10.0.10.2 │ │10.0.10.3 │ │10.0.10.4 │     │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │
│  │  Chroma  │ │ Sentinel │ │ Victoria │ │  Grafana │     │
│  │10.0.10.5 │ │10.0.10.6 │ │10.0.10.7 │ │10.0.10.8 │     │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │
│           │                                                  │
│           └────────── Aetheris Core ──────────► UDP 51820  │
│                                                             │
│  172.20.0.0/24 (aetheris_isolation_net - internal only)    │
│  ┌──────────────────┐                                       │
│  │   Ghost Shell    │  Honeypot                            │
│  │   172.20.0.10    │                                       │
│  └──────────────────┘                                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Services

| Service | Port | Description |
|---------|------|-------------|
| wireguard-mesh | 51820/udp | WireGuard VPN server |
| opa-gateway | 8181 | OPA policy engine |
| encrypted-vault | 8200 | HashiCorp Vault |
| aetheris-core | 8080 | Main application |
| ollama-ai | 11434 | Ollama LLM |
| chroma-vector | 8000 | Vector database |
| victoria-metrics | 8428 | Metrics storage |
| grafana | 3000 | Dashboard |

## Security Features

### Zero-Trust
- All requests evaluated by OPA policies
- JWT-based authentication
- Fine-grained authorization

### Encryption
- AES-256-GCM for ZFS emulation
- TLS for network communication
- Vault for secrets management

### Network Isolation
- Separate networks for production and honeypot
- No external access to internal services
- WireGuard for encrypted mesh

## Testing

### UAT Test Modules

1. **Prerequisites**: Docker, Docker Compose
2. **Build**: Build all container images
3. **Infrastructure**: Start core services
4. **Zero-Trust**: Verify OPA policies
5. **Encryption**: Verify encrypted storage
6. **Mesh**: Verify WireGuard network
7. **Isolation**: Verify network segmentation
8. **Core**: Verify Aetheris Core
9. **Monitoring**: Verify monitoring stack

### Running Tests

```bash
# Full UAT test suite
bash tests/run_emulation_uat.sh

# Manual testing
docker compose -f compose.emulation.yaml exec network-tester sh
```

## Volumes

All data is persisted in Docker volumes:

- `zfs_pool` - Simulated ZFS storage
- `vault_data` - Vault data directory
- `ollama_data` - AI model cache
- `chroma_data` - Vector embeddings
- `victoria_data` - Metrics data
- `grafana_data` - Dashboard configs

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| AI_ENDPOINT | http://10.0.10.4:11434 | Ollama endpoint |
| OPA_ENDPOINT | http://10.0.10.2:8181 | OPA endpoint |
| VAULT_ADDR | http://10.0.10.3:8200 | Vault endpoint |
| LOG_LEVEL | debug | Logging level |

## Notes

- This is for **testing only**
- Do not use in production
- All encryption keys are test keys
- Networks are isolated from host
