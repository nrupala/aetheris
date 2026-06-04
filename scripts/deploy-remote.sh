#!/bin/bash
set -e
cd /opt/aetheris

echo "=== Checking containers ==="
docker ps --format 'table {{.Names}}\t{{.Status}}'

echo "=== Pulling models ==="
for model in nomic-embed-text qwen3:8b deepseek-r1:8b phi4-mini bge-reranker-v2-m3; do
  docker exec aetheris_ollama ollama pull "$model" 2>/dev/null && echo "  $model OK" || echo "  $model SKIP"
done

echo "=== Health checks ==="
sleep 2
curl -sf http://localhost:8080/status | head -c 200
echo ""
curl -sf -u dev_user:BCjfTYIIjMASFGVM http://localhost:9080/api/health | head -c 150
echo ""
echo "Deploy complete"
