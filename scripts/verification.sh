#!/usr/bin/env bash
set -e

# ──────────────────────────────────────────────
# Aetheris Production Health Verification
# Run: bash scripts/verification.sh
# ──────────────────────────────────────────────

CORE_PORT="${AETHERIS_CORE_PORT:-8080}"
NGINX_HTTP_PORT="${NGINX_HTTP_PORT:-9080}"
NGINX_HTTPS_PORT="${NGINX_HTTPS_PORT:-9443}"
OPA_PORT="${OPA_GATEWAY_PORT:-8181}"
VICTORIA_PORT="${VICTORIA_METRICS_PORT:-8428}"
HTPASSWD_USER="${HTPASSWD_USER:-dev_user}"
HTPASSWD_PASS="${HTPASSWD_PASS:-BCjfTYIIjMASFGVM}"

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
echo "── Nginx ──"

if curl -sf -o /dev/null -w "" "http://localhost:${NGINX_HTTP_PORT}/" > /dev/null 2>&1; then
    pass "Nginx HTTP listener"
else
    fail "Nginx HTTP listener"
fi

if curl -sfk -o /dev/null "https://localhost:${NGINX_HTTPS_PORT}/" > /dev/null 2>&1; then
    pass "Nginx HTTPS listener"
else
    fail "Nginx HTTPS listener"
fi

echo ""
echo "── Dev Sandbox ──"

CURL_AUTH="-u ${HTPASSWD_USER}:${HTPASSWD_PASS}"

if curl -sfk ${CURL_AUTH} -H "Host: dev.nrupalakolkar.com" "http://localhost:${NGINX_HTTP_PORT}/" > /dev/null 2>&1; then
    pass "Dev page (auth)"
else
    fail "Dev page (auth)"
fi

HEALTH=$(curl -sfk ${CURL_AUTH} -H "Host: dev.nrupalakolkar.com" "http://localhost:${NGINX_HTTP_PORT}/api/health" 2>/dev/null)
if echo "$HEALTH" | grep -q '"status":"ok"'; then
    pass "Dev API health"
else
    fail "Dev API health"
fi

LOGS=$(curl -sfk ${CURL_AUTH} -H "Host: dev.nrupalakolkar.com" "http://localhost:${NGINX_HTTP_PORT}/api/dev/logs" 2>/dev/null)
if echo "$LOGS" | grep -q '"logs"'; then
    pass "Dev API logs"
else
    fail "Dev API logs"
fi

CONFIG=$(curl -sfk ${CURL_AUTH} -H "Host: dev.nrupalakolkar.com" "http://localhost:${NGINX_HTTP_PORT}/api/dev/config" 2>/dev/null)
if echo "$CONFIG" | grep -q '"port_registry.json"'; then
    pass "Dev API config"
else
    fail "Dev API config"
fi

METRICS=$(curl -sfk ${CURL_AUTH} -H "Host: dev.nrupalakolkar.com" "http://localhost:${NGINX_HTTP_PORT}/api/dev/metrics" 2>/dev/null)
if echo "$METRICS" | grep -q '"services"'; then
    pass "Dev API metrics"
else
    fail "Dev API metrics"
fi

echo ""
echo "── AI & RAG ──"

if curl -sfk ${CURL_AUTH} -H "Host: ai.nrupalakolkar.com" "http://localhost:${NGINX_HTTP_PORT}/" > /dev/null 2>&1; then
    pass "AI page (auth)"
else
    fail "AI page (auth)"
fi

if curl -sfk ${CURL_AUTH} -H "Host: rag.nrupalakolkar.com" "http://localhost:${NGINX_HTTP_PORT}/" > /dev/null 2>&1; then
    pass "RAG page (auth)"
else
    fail "RAG page (auth)"
fi

if curl -sfk ${CURL_AUTH} -H "Host: agents.nrupalakolkar.com" "http://localhost:${NGINX_HTTP_PORT}/" > /dev/null 2>&1; then
    pass "Agents page (auth)"
else
    fail "Agents page (auth)"
fi

STATUS=$(curl -sfk ${CURL_AUTH} -H "Host: agents.nrupalakolkar.com" "http://localhost:${NGINX_HTTP_PORT}/agents/status" 2>/dev/null)
if echo "$STATUS" | grep -q '"agents"'; then
    pass "Agents API"
else
    fail "Agents API"
fi

echo ""
echo "── Infrastructure ──"

if docker exec aetheris_core wget --spider -q http://aetheris_opa:8181/health 2>/dev/null; then
    pass "OPA health"
else
    fail "OPA health"
fi

if curl -sf "http://localhost:${VICTORIA_PORT}/health" > /dev/null 2>&1; then
    pass "VictoriaMetrics"
else
    fail "VictoriaMetrics"
fi

echo ""
echo "── Results ──"
echo "  Passed: ${PASSED}"
echo "  Failed: ${FAILED}"
echo "═══════════════════════════════════════════"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
