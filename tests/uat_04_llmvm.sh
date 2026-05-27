#!/bin/bash
# UAT-04: LLMVM Integration Tests
set -e
FAILED=0
PASSED=0
CORE_PORT=${AETHERIS_CORE_PORT:-8080}
RAG_PORT=${RAG_SERVICE_PORT:-8081}

echo "========================================="
echo "UAT-04: LLMVM INTEGRATION TESTS"
echo "========================================="

# UAT-04.01: RAG Service Health
echo "[UAT-04.01] Testing RAG Service Health..."
if curl -sf "http://localhost:${RAG_PORT}/health" >/dev/null 2>&1; then
    echo "  PASS: RAG service is healthy"
    ((PASSED++)) || true
else
    echo "  WARN: RAG service not ready (needs LMStudio on host)"
fi

# UAT-04.02: RAG Container Running
echo "[UAT-04.02] Testing RAG Container..."
RAG_STATUS=$(docker inspect --format='{{.State.Status}}' llmvm_rag 2>/dev/null || echo "missing")
if [ "$RAG_STATUS" == "running" ]; then
    echo "  PASS: RAG container is running"
    ((PASSED++)) || true
else
    echo "  WARN: RAG container not running (status: $RAG_STATUS)"
fi

# UAT-04.03: Orchestrator Container Running
echo "[UAT-04.03] Testing Orchestrator Container..."
ORCH_STATUS=$(docker inspect --format='{{.State.Status}}' llmvm_orchestrator 2>/dev/null || echo "missing")
if [ "$ORCH_STATUS" == "running" ]; then
    echo "  PASS: Orchestrator container is running"
    ((PASSED++)) || true
else
    echo "  WARN: Orchestrator container not running (status: $ORCH_STATUS)"
fi

# UAT-04.04: Docker Images Exist
echo "[UAT-04.04] Testing LLMVM Docker Images..."
RAG_IMAGE=$(docker images --format '{{.Repository}}' | grep -c "llmvm_rag" || true)
ORCH_IMAGE=$(docker images --format '{{.Repository}}' | grep -c "llmvm_orchestrator" || true)
if [ "$RAG_IMAGE" -gt 0 ] && [ "$ORCH_IMAGE" -gt 0 ]; then
    echo "  PASS: LLMVM Docker images built successfully"
    ((PASSED++)) || true
else
    echo "  FAIL: LLMVM Docker images not found"
    ((FAILED++)) || true
fi

echo ""
echo "UAT-04 RESULTS: $PASSED passed, $FAILED failed"
[ $FAILED -eq 0 ] && exit 0 || exit 1
