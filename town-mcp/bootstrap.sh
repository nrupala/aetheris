#!/usr/bin/env bash
# Phase 1 bootstrap for the Town Sovereign MCP server. No Docker.
# Run ON oracle-aetheris, from the town-mcp/ directory, as the user that should
# own the service.
set -euo pipefail

APP_DIR="${APP_DIR:-/opt/town-mcp}"
RUN_USER="${RUN_USER:-$(whoami)}"
PY="${PY:-python3}"

echo "==> Installing Town Sovereign MCP to ${APP_DIR} (user: ${RUN_USER})"

sudo mkdir -p "${APP_DIR}"
sudo chown "${RUN_USER}:${RUN_USER}" "${APP_DIR}"

# Copy repo contents into APP_DIR (excluding local venv/data)
rsync -a --exclude '.venv' --exclude 'data' --exclude '__pycache__' ./ "${APP_DIR}/"

cd "${APP_DIR}"
"${PY}" -m venv .venv
./.venv/bin/pip install --upgrade pip >/dev/null
./.venv/bin/pip install -r requirements.txt
mkdir -p data

# .env with a strong generated bearer token
if [ ! -f .env ]; then
  cp .env.example .env
  TOKEN="$(head -c 48 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 48)"
  sed -i "s|^MCP_BEARER_TOKEN=.*|MCP_BEARER_TOKEN=${TOKEN}|" .env
  echo "==> Generated MCP_BEARER_TOKEN in ${APP_DIR}/.env"
fi

# Ensure the local models are present (Ollama already serving on this box).
# EMBED_MODEL powers sovereign_search; CHAT_MODEL powers rag_answer.
EMBED_MODEL="$(grep -E '^EMBED_MODEL=' .env | cut -d= -f2)"
CHAT_MODEL="$(grep -E '^CHAT_MODEL=' .env | cut -d= -f2)"
if command -v ollama >/dev/null 2>&1; then
  for M in "${EMBED_MODEL}" "${CHAT_MODEL}"; do
    if [ -n "${M}" ]; then
      echo "==> Pulling Ollama model: ${M}"
      ollama pull "${M}" || echo "WARN: could not pull ${M}; pull it manually"
    fi
  done
else
  echo "WARN: 'ollama' CLI not on PATH -- ensure Ollama serves on 127.0.0.1:11434"
fi

# Write the live systemd unit for this user
UNIT=/etc/systemd/system/town-mcp.service
sudo tee "${UNIT}" >/dev/null <<EOF
[Unit]
Description=Town Sovereign MCP server (Ollama-backed sovereign_search + rag_answer)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${RUN_USER}
WorkingDirectory=${APP_DIR}
EnvironmentFile=${APP_DIR}/.env
ExecStart=${APP_DIR}/.venv/bin/python -m uvicorn src.server:app --host \${HOST} --port \${PORT}
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now town-mcp.service

PORT="$(grep -E '^PORT=' .env | cut -d= -f2)"
echo "==> Service started."
echo "    Health:  curl -s http://127.0.0.1:${PORT}/health"
echo "    Index :  ${APP_DIR}/.venv/bin/python -m src.indexer --corpus /path/to/corpus --clear"
echo "==> Next: add a cloudflared ingress route (e.g. mcp.devinfo.dev -> http://127.0.0.1:${PORT}),"
echo "    put it behind a Cloudflare Access service-token policy, then register the URL in Town (Settings -> MCP)."
