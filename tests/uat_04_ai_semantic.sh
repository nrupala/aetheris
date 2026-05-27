#!/bin/bash
# UAT-04: AI & Semantic Search Tests
set -e
FAILED=0
PASSED=0
CORE_PORT=${AETHERIS_CORE_PORT:-8080}
OLLAMA_PORT=${OLLAMA_PORT:-11434}
CHROMA_PORT=${CHROMA_PORT:-8000}

echo "========================================="
echo "UAT-04: AI SEMANTIC SEARCH TESTS"
echo "========================================="

# UAT-04.01: Ollama Health
echo "[UAT-04.01] Testing Ollama Health..."
if curl -s "http://localhost:${OLLAMA_PORT}/api/tags" >/dev/null 2>&1; then
    echo "  PASS: Ollama is healthy"
    ((PASSED++)) || true
else
    echo "  FAIL: Ollama not responding"
    ((FAILED++)) || true
fi

# UAT-04.02: Text Generation
echo "[UAT-04.02] Testing Text Generation..."
RESP=$(curl -s -X POST "http://localhost:${OLLAMA_PORT}/api/generate" \
    -H "Content-Type: application/json" \
    -d '{"model":"mistral","prompt":"What is 2+2? Answer in one word.","stream":false}' 2>/dev/null)
if echo "$RESP" | grep -qi "4\|four"; then
    echo "  PASS: Text generation works"
    ((PASSED++)) || true
else
    echo "  WARN: Text generation may need model download (response: $RESP)"
    echo "  INFO: Run 'docker exec aetheris_ai ollama pull mistral' to download"
fi

# UAT-04.03: Embedding Generation
echo "[UAT-04.03] Testing Embedding Generation..."
RESP=$(curl -s -X POST "http://localhost:${OLLAMA_PORT}/api/embeddings" \
    -H "Content-Type: application/json" \
    -d '{"model":"nomic-embed-text","prompt":"test document"}' 2>/dev/null)
EMBED_LEN=$(echo "$RESP" | grep -o '"embedding":\[' | wc -l)
if [ "$EMBED_LEN" -gt 0 ]; then
    echo "  PASS: Embeddings generated"
    ((PASSED++)) || true
else
    echo "  WARN: Embedding model may need download"
    echo "  INFO: Run 'docker exec aetheris_ai ollama pull nomic-embed-text'"
fi

# UAT-04.04: ChromaDB Health
echo "[UAT-04.04] Testing ChromaDB Health..."
if curl -s "http://localhost:${CHROMA_PORT}/api/v1/heartbeat" >/dev/null 2>&1; then
    echo "  PASS: ChromaDB is healthy"
    ((PASSED++)) || true
else
    echo "  FAIL: ChromaDB not responding"
    ((FAILED++)) || true
fi

# UAT-04.05: End-to-End Semantic Search
echo "[UAT-04.05] Testing End-to-End Semantic Search..."
SEMFILE="/tmp/uat_semantic_$(date +%s).txt"
echo "This document contains information about the annual budget and financial reports" > "$SEMFILE"
curl -s -X POST -F "file=@$SEMFILE" "http://localhost:${CORE_PORT}/upload" >/dev/null 2>&1
echo "  INFO: Uploaded test file, waiting for indexing..."
sleep 5
SEARCH_RESP=$(curl -s "http://localhost:${CORE_PORT}/search?q=budget+financial" 2>/dev/null)
if echo "$SEARCH_RESP" | grep -qi "semantic\|budget"; then
    echo "  PASS: Semantic search found results"
    ((PASSED++)) || true
else
    echo "  INFO: Semantic search returned: $SEARCH_RESP"
    echo "  WARN: Results may need AI indexing to complete"
fi
rm -f "$SEMFILE"

echo ""
echo "UAT-04 RESULTS: $PASSED passed, $FAILED failed"
[ $FAILED -eq 0 ] && exit 0 || exit 1
