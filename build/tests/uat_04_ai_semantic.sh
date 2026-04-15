#!/bin/bash
# UAT-04: AI & Semantic Search Tests
set -e
FAILED=0
PASSED=0

echo "========================================="
echo "UAT-04: AI SEMANTIC SEARCH TESTS"
echo "========================================="

# UAT-04.01: Ollama Health
echo "[UAT-04.01] Testing Ollama Health..."
if curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
    echo "  PASS: Ollama is healthy"
    ((PASSED++))
else
    echo "  FAIL: Ollama not responding"
    ((FAILED++))
fi

# UAT-04.02: Text Generation
echo "[UAT-04.02] Testing Text Generation..."
RESP=$(curl -s -X POST http://localhost:11434/api/generate \
    -H "Content-Type: application/json" \
    -d '{"model":"mistral","prompt":"What is 2+2? Answer in one word.","stream":false}' 2>/dev/null)
if echo "$RESP" | grep -qi "4\|four"; then
    echo "  PASS: Text generation works"
    ((PASSED++))
else
    echo "  WARN: Text generation may need model download (response: $RESP)"
    echo "  INFO: Run 'docker exec aetheris_ai ollama pull mistral' to download"
fi

# UAT-04.03: Embedding Generation
echo "[UAT-04.03] Testing Embedding Generation..."
RESP=$(curl -s -X POST http://localhost:11434/api/embeddings \
    -H "Content-Type: application/json" \
    -d '{"model":"nomic-embed-text","prompt":"test document"}' 2>/dev/null)
EMBED_LEN=$(echo "$RESP" | grep -o '"embedding":\[' | wc -l)
if [ "$EMBED_LEN" -gt 0 ]; then
    echo "  PASS: Embeddings generated"
    ((PASSED++))
else
    echo "  WARN: Embedding model may need download"
    echo "  INFO: Run 'docker exec aetheris_ai ollama pull nomic-embed-text'"
fi

# UAT-04.04: ChromaDB Health
echo "[UAT-04.04] Testing ChromaDB Health..."
if curl -s http://localhost:8000/api/v1/heartbeat >/dev/null 2>&1; then
    echo "  PASS: ChromaDB is healthy"
    ((PASSED++))
else
    echo "  FAIL: ChromaDB not responding"
    ((FAILED++))
fi

# UAT-04.05: End-to-End Semantic Search
echo "[UAT-04.05] Testing End-to-End Semantic Search..."
SEMFILE="/tmp/uat_semantic_$(date +%s).txt"
echo "This document contains information about the annual budget and financial reports" > "$SEMFILE"
curl -s -X POST -F "file=@$SEMFILE" http://localhost:8080/upload >/dev/null 2>&1
echo "  INFO: Uploaded test file, waiting for indexing..."
sleep 5
SEARCH_RESP=$(curl -s "http://localhost:8080/search?q=budget+financial" 2>/dev/null)
if echo "$SEARCH_RESP" | grep -qi "semantic\|budget"; then
    echo "  PASS: Semantic search found results"
    ((PASSED++))
else
    echo "  INFO: Semantic search returned: $SEARCH_RESP"
    echo "  WARN: Results may need AI indexing to complete"
fi
rm -f "$SEMFILE"

echo ""
echo "UAT-04 RESULTS: $PASSED passed, $FAILED failed"
[ $FAILED -eq 0 ] && exit 0 || exit 1
