#!/usr/bin/env bash
# Aetheris - native (no-Docker) bootstrap with Docker->native handover + rollback.
# Run ON oracle-aetheris from the repo checkout (default /opt/aetheris).
# Idempotent. Stops the Docker core+nginx, starts native systemd core + system
# nginx, verifies ai_connected, and ROLLS BACK to Docker if the native stack is
# unhealthy (no-lockout). Ollama + cloudflared stay native and untouched.
#
# NOTE: intentionally NOT `set -e` — errors are handled explicitly so a failed
# cutover rolls back to Docker instead of leaving the box half-migrated.
set -uo pipefail

APP_DIR="${APP_DIR:-/opt/aetheris}"
RUN_USER="${RUN_USER:-$(whoami)}"
BIN="${BIN:-/usr/local/bin/aetheris-core}"
WEB_ROOT="${WEB_ROOT:-/usr/share/nginx/html}"
SSL_DIR="${SSL_DIR:-/etc/nginx/ssl}"

log(){ echo "==> $*"; }

rollback_to_docker(){
  echo "ERROR: native stack unhealthy — rolling back to Docker." >&2
  sudo systemctl stop aetheris-core.service 2>/dev/null || true
  sudo systemctl disable aetheris-core.service 2>/dev/null || true
  if command -v docker >/dev/null 2>&1; then
    (cd "$APP_DIR" && docker compose up -d aetheris-core nginx) || true
  fi
  echo "Rolled back. Investigate before retrying." >&2
  exit 1
}

cd "$APP_DIR"
log "Aetheris native bootstrap (user: $RUN_USER, app: $APP_DIR)"

# 0. Preconditions
[ -x "$BIN" ] || { echo "ERROR: $BIN missing/not executable (the deploy ships it)"; exit 1; }
mkdir -p "$APP_DIR/vault"

# 1. Runtime env — 127.0.0.1 endpoints (no Docker DNS names)
[ -f "$APP_DIR/.env.aetheris" ] || cp "$APP_DIR/.env.aetheris.example" "$APP_DIR/.env.aetheris"

# 2. Ensure Ollama models present (native ollama on 127.0.0.1:11434)
if command -v ollama >/dev/null 2>&1; then
  for m in nomic-embed-text qwen3:8b deepseek-r1:8b phi4-mini bge-reranker-v2-m3; do
    ollama pull "$m" >/dev/null 2>&1 && echo "  model $m: OK" || echo "  model $m: SKIP"
  done
else
  echo "WARN: 'ollama' CLI not on PATH — ensure ollama.service serves 127.0.0.1:11434"
fi

# 3. nginx config: HTTP(9080) always; HTTPS(9443, root domain + git) only if certs exist
if command -v nginx >/dev/null 2>&1; then
  sudo cp "$APP_DIR/nginx/default.native.conf" /etc/nginx/conf.d/default.conf
  [ -f "$APP_DIR/nginx/.htpasswd" ] && sudo cp "$APP_DIR/nginx/.htpasswd" /etc/nginx/.htpasswd
  if [ -f "$SSL_DIR/fullchain.cer" ] && [ -f "$SSL_DIR/nrupalakolkar.com.key" ]; then
    sudo cp "$APP_DIR/nginx/ssl.native.conf" /etc/nginx/conf.d/ssl.conf
    log "SSL certs found at $SSL_DIR — installed ssl.native.conf (9443)"
  else
    echo "ERROR: no TLS certs at $SSL_DIR (need fullchain.cer + nrupalakolkar.com.key)."
    echo "       Refusing to hand over: without 9443, the root domain + git would break."
    echo "       Place the certs, then re-run. Docker left running; box unchanged."
    exit 1
  fi
  sudo mkdir -p "$WEB_ROOT"; sudo cp -r "$APP_DIR/web/." "$WEB_ROOT/"
  sudo nginx -t || { echo "nginx config invalid"; rollback_to_docker; }
fi

# 4. Handover: stop Docker core + nginx to free ports 8080 / 9080 / 9443
if command -v docker >/dev/null 2>&1; then
  log "Stopping Docker aetheris-core + nginx (handover)"
  (cd "$APP_DIR" && docker compose stop aetheris-core nginx) || true
  sleep 2
fi

# 5. systemd unit + start native core (User substituted for this box)
UNIT=/etc/systemd/system/aetheris-core.service
sudo tee "$UNIT" >/dev/null <<EOF
[Unit]
Description=Aetheris Core (Rust orchestrator, native — no Docker)
Documentation=https://github.com/nrupala/aetheris
After=network-online.target ollama.service
Wants=network-online.target

[Service]
Type=simple
User=$RUN_USER
WorkingDirectory=$APP_DIR/core
EnvironmentFile=$APP_DIR/.env.aetheris
ExecStart=$BIN
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable --now aetheris-core.service
sudo systemctl restart aetheris-core.service
if command -v nginx >/dev/null 2>&1; then sudo systemctl enable nginx; sudo systemctl restart nginx; fi

# 6. Verify — roll back to Docker if AI is not actually connected
sleep 4
HEALTH="$(curl -sf http://127.0.0.1:8080/health || true)"
echo "health: $HEALTH"
echo "$HEALTH" | grep -q '"ai_connected":true' || rollback_to_docker
log "Native stack healthy: ai_connected=true. Cutover complete."
