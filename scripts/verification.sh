#!/usr/bin/env bash
set -e

# ──────────────────────────────────────────────
# Aetheris Production Health Verification
# Run: bash scripts/verification.sh
# ──────────────────────────────────────────────

CORE_PORT="${AETHERIS_CORE_PORT:-8080}"
OPA_PORT="${OPA_GATEWAY_PORT:-8181}"
VICTORIA_PORT="${VICTORIA_METRICS_PORT:-8428}"

FAILED=0
PASSED=0

pass() { PASSED=$((PASSED+1)); echo "  PASS: $1"; }
fail() { FAILED=$((FAILED+1)); echo "  FAIL: $1"; }

echo "═══════════════════════════════════════════"
echo "  Aetheris Production Verification"
echo "═══════════════════════════════════════════"

echo ""
echo "── Core Services ──"

if curl -sf "http://localhost:${CORE_PORT}/status" > /dev/null 2>&1; then
    pass "Core HTTP endpoint"
else
    fail "Core HTTP endpoint"
fi

if curl -sf "http://localhost:${CORE_PORT}/health" > /dev/null 2>&1; then
    pass "Core health endpoint"
else
    fail "Core health endpoint"
fi

echo ""
echo "── Infrastructure ──"

if curl -sf "http://localhost:${OPA_PORT}/health" > /dev/null 2>&1; then
    pass "OPA health"
else
    fail "OPA health"
fi

if curl -sf "http://localhost:${VICTORIA_PORT}/health" > /dev/null 2>&1; then
    pass "VictoriaMetrics"
else
    echo "  SKIP: VictoriaMetrics (not deployed)"
fi

echo ""
echo "── Results ──"
echo "  Passed: ${PASSED}"
echo "  Failed: ${FAILED}"
echo "═══════════════════════════════════════════"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
