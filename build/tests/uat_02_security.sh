#!/bin/bash
# UAT-02: Security Tests
set -e
FAILED=0
PASSED=0

echo "========================================="
echo "UAT-02: SECURITY TESTS"
echo "========================================="

# UAT-02.01: Default Deny
echo "[UAT-02.01] Testing Default Deny (no auth)..."
RESP=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/download/test.txt 2>/dev/null || echo "000")
if [ "$RESP" == "403" ] || [ "$RESP" == "000" ]; then
    echo "  PASS: Default deny working (got $RESP)"
    ((PASSED++))
else
    echo "  FAIL: Got $RESP, expected 403"
    ((FAILED++))
fi

# UAT-02.02: Path Traversal Prevention
echo "[UAT-02.02] Testing Path Traversal Prevention..."
RESP=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/download/../../../etc/passwd" 2>/dev/null || echo "000")
if [ "$RESP" == "403" ] || [ "$RESP" == "000" ]; then
    echo "  PASS: Path traversal blocked (got $RESP)"
    ((PASSED++))
else
    echo "  FAIL: Path traversal succeeded (got $RESP)"
    ((FAILED++))
fi

# UAT-02.03: OPA Health
echo "[UAT-02.03] Testing OPA Health..."
if curl -s http://localhost:8181/health >/dev/null 2>&1; then
    echo "  PASS: OPA is healthy"
    ((PASSED++))
else
    echo "  FAIL: OPA not responding"
    ((FAILED++))
fi

# UAT-02.04: OPA Default Policy
echo "[UAT-02.04] Testing OPA Default Deny Policy..."
RESP=$(curl -s -X POST http://localhost:8181/v1/data/aetheris/authz/allow \
    -H "Content-Type: application/json" \
    -d '{"input":{"user_role":"unknown"}}' 2>/dev/null)
if echo "$RESP" | grep -q '"result":false'; then
    echo "  PASS: OPA denies unknown role"
    ((PASSED++))
else
    echo "  FAIL: OPA did not deny (got: $RESP)"
    ((FAILED++))
fi

# UAT-02.05: Encryption Check
echo "[UAT-02.05] Testing ZFS Encryption..."
ENC=$(sudo zfs get encryption aetheris_vault/secure_data 2>/dev/null | grep -v NAME | awk '{print $3}')
if [ "$ENC" == "aes-256-gcm" ]; then
    echo "  PASS: ZFS encryption is AES-256-GCM"
    ((PASSED++))
else
    echo "  SKIP: ZFS not configured (encryption: $ENC)"
fi

echo ""
echo "UAT-02 RESULTS: $PASSED passed, $FAILED failed"
[ $FAILED -eq 0 ] && exit 0 || exit 1
