#!/bin/bash
# Native (no-Docker) on-box refresh helper. Run from /opt/aetheris.
# Pulls models via native ollama and reports core health.
# NOTE: the previous version hardcoded basic-auth credentials in cleartext —
# removed. Health is checked unauthenticated on localhost.
set -e
cd /opt/aetheris

echo "=== aetheris-core service ==="
systemctl --no-pager status aetheris-core.service | head -n 5 || true

echo "=== Pulling models (native ollama) ==="
for model in nomic-embed-text qwen3:8b deepseek-r1:8b phi4-mini bge-reranker-v2-m3; do
  ollama pull "$model" >/dev/null 2>&1 && echo "  $model OK" || echo "  $model SKIP"
done

echo "=== Health check (unauthenticated, localhost) ==="
sleep 2
curl -sf http://127.0.0.1:8080/health | head -c 300
echo ""
echo "Refresh complete"
