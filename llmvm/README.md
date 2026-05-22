# Aetheris LLM VM
Oracle Cloud Always Free ARM server + Cloudflare Tunnel for remote AI inference.

## Architecture
```
Aetheris Core (local) ──HTTPS/TLS──▶ Cloudflare Tunnel ──▶ Oracle VM (Ampere A1)
                                                        ──▶ LMStudio Server
                                                        ──▶ AI Models
```

## Oracle VM Specs (Always Free)
- **Shape**: VM.Standard.A1.Flex
- **CPU**: 4 OCPUs (ARM64 / Ampere Altra)
- **RAM**: 24 GB
- **Storage**: 200 GB total (boot + data)
- **OS**: Ubuntu 22.04 LTS ARM64
- **Network**: Private subnet + Cloudflare Tunnel (no public IP needed)

## Models That Fit
| Model | Size | VRAM | Status |
|-------|------|------|--------|
| nvidia/nemotron-3-nano-4b | 2.8 GB | 4 GB | ✅ Fast |
| essentialai/rnj-1 | 5.1 GB | 7 GB | ✅ Good |
| google/gemma-3-1b | ~1 GB | 2 GB | ✅ Ultra fast |
| mistralai/ministral-3-3b | ~2 GB | 3 GB | ✅ Good |
| strand-rust-coder-14b-v1 | ~8 GB | 12 GB | ⚠️ Heavy |
| microsoft/phi-4-reasoning-plus | ~5 GB | 7 GB | ✅ Reasoning |

## Setup
1. Create Oracle VM (see `docs/ORACLE_SETUP.md`)
2. Run `./setup.sh` on the VM
3. Configure Cloudflare Tunnel (see `docs/CLOUDFLARE_SETUP.md`)
4. Update Aetheris config with tunnel URL
5. Test: `curl https://ai.your-domain.com/v1/models`
