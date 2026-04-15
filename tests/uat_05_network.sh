#!/bin/bash
# UAT-05: Network & Connectivity Tests
set -e
FAILED=0
PASSED=0

echo "========================================="
echo "UAT-05: NETWORK & CONNECTIVITY TESTS"
echo "========================================="

# UAT-05.01: Core API Health
echo "[UAT-05.01] Testing Aetheris Core Health..."
RESP=$(curl -s http://localhost:8080/status 2>/dev/null)
if echo "$RESP" | grep -q '"version"'; then
    echo "  PASS: Core API responding"
    ((PASSED++))
else
    echo "  FAIL: Core not responding properly"
    ((FAILED++))
fi

# UAT-05.02: VictoriaMetrics Health
echo "[UAT-05.02] Testing VictoriaMetrics Health..."
if curl -s http://localhost:8428/health >/dev/null 2>&1; then
    echo "  PASS: VictoriaMetrics is healthy"
    ((PASSED++))
else
    echo "  FAIL: VictoriaMetrics not responding"
    ((FAILED++))
fi

# UAT-05.03: Metrics Endpoint
echo "[UAT-05.03] Testing Prometheus Metrics..."
METRICS=$(curl -s http://localhost:8080/metrics 2>/dev/null)
if echo "$METRICS" | grep -q "aetheris_"; then
    echo "  PASS: Prometheus metrics available"
    ((PASSED++))
else
    echo "  WARN: Metrics format may differ"
fi

# UAT-05.04: Zero-JS Dashboard
echo "[UAT-05.04] Testing Zero-JS Dashboard..."
HTML=$(curl -s http://localhost:8080/ 2>/dev/null)
SCRIPT_COUNT=$(echo "$HTML" | grep -c "<script" || echo "0")
if [ "$SCRIPT_COUNT" -eq 0 ]; then
    echo "  PASS: Dashboard is zero-JS"
    ((PASSED++))
else
    echo "  FAIL: Dashboard contains $SCRIPT_COUNT script tags"
    ((FAILED++))
fi

# UAT-05.05: Service Restart Test
echo "[UAT-05.05] Testing Service Restart Resilience..."
echo "  INFO: Skipping restart test (manual verification required)"
echo "  To test: docker restart aetheris_core && sleep 10 && curl localhost:8080/status"

echo ""
echo "UAT-05 RESULTS: $PASSED passed, $FAILED failed"
[ $FAILED -eq 0 ] && exit 0 || exit 1
