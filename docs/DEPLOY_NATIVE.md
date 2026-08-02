# Deploying Aetheris Core - Native (no Docker)

This is the canonical deployment for Aetheris Core after the Docker -> native cutover.
The core compiles to a single static Rust binary (`aetheris-core`) that listens on
`127.0.0.1:8080`. It runs directly on the host under **systemd** - no Docker, no
compose, no nginx. Public access is via **cloudflared** pointing straight at the
loopback port, and the public hostname `core.nrupalakolkar.com` is gated by
**Cloudflare Access** (no HTTP Basic Auth). Ollama runs loopback-only at
`127.0.0.1:11434` and the core reaches it directly (no Docker bridge, no 172.28.0.1).

> Nothing in this repo deploys automatically. A human merges the PR, then runs
> `scripts/install-native.sh` on the box.

## 1. Prerequisites
- A Linux host (the `oracle-aetheris` OCI ARM box or similar), `x86_64` or `aarch64`.
- Rust toolchain via `rustup`, with the matching musl target available
  (`x86_64-unknown-linux-musl` or `aarch64-unknown-linux-musl`). The installer adds
  the target automatically and falls back to the host-native build if musl is missing.
- `systemd` and root/sudo on the host.
- **Ollama** installed and listening on `127.0.0.1:11434`, with the required models
  pulled: at minimum `qwen2.5:7b` (generation) and `nomic-embed-text` (embeddings);
  `qwen3:8b` for deep reasoning.
- `cloudflared` installed and authenticated to the Cloudflare account that owns
  `nrupalakolkar.com`.

## 2. Build
The installer builds the binary for you. To build by hand:
```bash
cd core
# static musl (what Dockerfile.core used to produce)
rustup target add x86_64-unknown-linux-musl   # or aarch64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
# -> core/target/x86_64-unknown-linux-musl/release/aetheris-core
```

## 3. Install (`scripts/install-native.sh`)
Run as root from the repo root after `git pull`:
```bash
sudo scripts/install-native.sh
```
The installer is idempotent and:
1. Builds the static `aetheris-core` (musl, falling back to host-native).
2. Creates the dedicated system user/group `aetheris` (nologin).
3. Installs the binary to `/opt/aetheris/bin/aetheris-core`.
4. Creates `/data` and `/data/vault` owned by `aetheris` (matches the unit's
   `ReadWritePaths=/data`).
5. Installs `/etc/aetheris/core.env` from `config/core.env.example` **only if it does
   not already exist** (an existing env file is never overwritten).
6. Installs the systemd unit to `/etc/systemd/system/aetheris-core.service`.
7. Runs `systemctl daemon-reload && systemctl enable --now aetheris-core`.

Environment (see `config/core.env.example`) - no secrets:

| Var | Default | Purpose |
|-----|---------|---------|
| `AI_ENDPOINT` | `http://127.0.0.1:11434` | Ollama (loopback) |
| `AETHERIS_FALLBACK_MODEL` | `qwen2.5:7b` | Generation fallback |
| `AETHERIS_EMBED_FALLBACK_MODEL` | `nomic-embed-text` | Embedding fallback |
| `PORT` | `8080` | Loopback HTTP port |
| `WEB_ROOT` | `/opt/aetheris/web` | Web panels root (per-subdomain `index.html`) |
| `OPA_ENDPOINT` | `http://127.0.0.1:8181` | OPA policy engine (loopback, optional) |
| `VAULT_PATH` | `/data/vault` | Vault / WAL directory |

## 4. Web panels & OpenAI-compat API

The core serves the web panels itself by **Host header** (no nginx): the `ai.*`,
`rag.*`, `agents.*`, `dev.*` (and `guardian.*`, `settings.*`) subdomains map to
`{WEB_ROOT}/<subdomain>/index.html`; any other host (apex, `core.*`, localhost)
gets `{WEB_ROOT}/index.html`. The panels are single-file HTML that call the core
directly, so they work from any subdomain.

The core also exposes an **OpenAI-compatible API at `/v1/*`** that reverse-proxies
to Ollama at `{AI_ENDPOINT}/v1/...` (e.g. `/v1/chat/completions`), streaming the
upstream body so `stream: true` works. `/v1/models` returns Ollama's real model
list. All JSON API routes are additionally reachable under `/api/...` (used by the
dev panel), e.g. `/api/health`.

## 5. cloudflared ingress
Point the tunnel straight at the loopback core (no nginx in between). Example
`/etc/cloudflared/config.yml`:
```yaml
tunnel: <your-tunnel-id>
credentials-file: /etc/cloudflared/<your-tunnel-id>.json
ingress:
  - hostname: core.nrupalakolkar.com
    service: http://127.0.0.1:8080
  - service: http_status:404
```
Route DNS and restart cloudflared:
```bash
cloudflared tunnel route dns <tunnel-name> core.nrupalakolkar.com
systemctl restart cloudflared
```
Ollama (`127.0.0.1:11434`) is **never** added to the ingress - it stays loopback-only.

## 6. Cloudflare Access (auth)
Auth is enforced by Cloudflare Access at the edge - there is no HTTP Basic Auth and no
`.htpasswd`.
1. In the Cloudflare Zero Trust dashboard, create **Access -> Applications -> Add**
   (Self-hosted).
2. Application domain: `core.nrupalakolkar.com`.
3. Add a policy that allows only your identity (email), and, for scripted/API access,
   a **service token**.
4. For non-interactive/API calls, send the service token as headers (the values live
   in your secret store, never in this repo):
   ```bash
   curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" \
        -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
        https://core.nrupalakolkar.com/api/health
   ```

## 7. Verification
```bash
# Service is up
systemctl is-active aetheris-core
systemctl status aetheris-core --no-pager
journalctl -u aetheris-core -n 50 --no-pager

# Health on the loopback (on the box)
curl -sf http://127.0.0.1:8080/health

# Per-subdomain web panel (Host-header routed; default host gets web_root/index.html)
curl -sf -H 'Host: ai.nrupalakolkar.com' http://127.0.0.1:8080/ | head
curl -sf -H 'Host: rag.nrupalakolkar.com' http://127.0.0.1:8080/ | grep -o '<title>[^<]*</title>'

# OpenAI-compat API through the core (proxied to loopback Ollama)
curl -sf http://127.0.0.1:8080/v1/models
curl -sf http://127.0.0.1:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"qwen2.5:7b","messages":[{"role":"user","content":"ping"}],"max_tokens":16}'

# JSON API is also reachable under the /api/ prefix (dev panel)
curl -sf http://127.0.0.1:8080/api/health

# One generation through loopback Ollama
curl -sf http://127.0.0.1:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"qwen2.5:7b","messages":[{"role":"user","content":"ping"}],"max_tokens":16}'

# One embedding through loopback Ollama
curl -sf http://127.0.0.1:11434/api/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"nomic-embed-text","prompt":"hello"}'

# Public path (off-box), through Cloudflare Access with a service token
curl -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" \
     -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
     https://core.nrupalakolkar.com/health
```

## 8. Teardown of the old Docker stack
Once the native service is verified, remove the retired Docker deployment. From a
checkout that still had `compose.yaml` (pre-cutover), or by container name:
```bash
docker compose down --remove-orphans
# Remove leftover containers if any survived
docker rm -f aetheris_core aetheris_nginx 2>/dev/null || true
# Optional: reclaim images (published to GHCR by the deleted deploy.yml)
docker image rm ghcr.io/nrupala/aetheris:latest ghcr.io/nrupala/aetheris-nginx:latest 2>/dev/null || true
```
nginx is gone entirely - cloudflared talks to the core directly, and Cloudflare Access
replaces HTTP Basic Auth.

## 9. Rollback
The native service is self-contained:
```bash
# Stop and disable the native service
sudo systemctl disable --now aetheris-core

# If you must temporarily return to Docker, check out a pre-cutover commit that still
# contains compose.yaml / Dockerfile.core and bring the stack back up:
git checkout <pre-cutover-sha>
docker compose up -d
```
Because the installer never overwrites an existing `/etc/aetheris/core.env`, your
config survives a reinstall. Re-running `scripts/install-native.sh` cleanly
re-installs the binary and unit.
