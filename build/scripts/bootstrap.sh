#!/usr/bin/env bash
set -e

echo "Initializing Aetheris: The Sovereign AI Mesh"

command -v docker >/dev/null 2>&1 || { echo "Docker/Podman required."; exit 1; }

echo "Creating directory structure..."
mkdir -p config/wireguard config/policy data/ollama data/chroma data/victoria vault core/ui scripts/ghost/dummy_vault

echo "Generating master encryption key..."
if [ ! -f .aetheris_key ]; then
    openssl rand -hex 32 > .aetheris_key
    chmod 600 .aetheris_key
    echo "Master encryption key generated."
fi

echo "Creating OPA policy..."
cat <<EOF > config/policy/policy.rego
package aetheris.authz
default allow = false
allow { input.user_role == "admin" }
allow { input.user_role == "analyst"; input.method == "GET" }
EOF

echo "Building containers..."
docker compose build --no-cache

echo "Launching Aetheris nodes..."
docker compose up -d

echo "Waiting for services..."
sleep 10

echo "Checking service health..."
curl -s http://localhost:8181/health > /dev/null && echo "OPA: OK" || echo "OPA: FAILED"
curl -s http://localhost:11434/api/tags > /dev/null && echo "Ollama: OK" || echo "Ollama: FAILED"
curl -s http://localhost:8000/api/v1/heartbeat > /dev/null && echo "ChromaDB: OK" || echo "ChromaDB: FAILED"

echo "System Online."
echo "Access via WireGuard Peer only."
