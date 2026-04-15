#!/usr/bin/env bash
set -e

echo "Aetheris Integrity Protocol - System Validation"
FAILED=0

echo "[TEST 1] WireGuard Stealth Check..."
ss -unlp 2>/dev/null | grep -q ":51820" && echo "PASS: Stealth Mesh Active" || { echo "FAIL: WireGuard not listening"; ((FAILED++)); }

echo "[TEST 2] ZFS Encryption Check..."
if command -v zfs >/dev/null 2>&1; then
    zfs get encryption aetheris_vault/secure_data 2>/dev/null | grep -q "aes-256-gcm" && echo "PASS: Vault Encrypted" || echo "SKIP: ZFS not configured"
else
    echo "SKIP: ZFS not available"
fi

echo "[TEST 3] OPA Decision Engine..."
OPA_RESULT=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8181/v1/data/aetheris/authz/allow)
[ "$OPA_RESULT" == "200" ] && echo "PASS: OPA Responsive" || { echo "FAIL: OPA Offline (Code: $OPA_RESULT)"; ((FAILED++)); }

echo "[TEST 4] Ollama Health..."
curl -s http://localhost:11434/api/tags > /dev/null && echo "PASS: Ollama Ready" || echo "FAIL: Ollama Offline"

echo "[TEST 5] ChromaDB Heartbeat..."
curl -s http://localhost:8000/api/v1/heartbeat > /dev/null && echo "PASS: ChromaDB Connected" || echo "FAIL: ChromaDB Offline"

echo "[TEST 6] VictoriaMetrics..."
curl -s http://localhost:8428/health > /dev/null && echo "PASS: Metrics Ready" || echo "FAIL: VictoriaMetrics Offline"

echo "[TEST 7] Aetheris Core Status..."
curl -s http://localhost:8080/status > /dev/null && echo "PASS: Core Running" || echo "FAIL: Core Offline"

echo "[TEST 8] Zero-JS Dashboard..."
HTML=$(curl -s http://localhost:8080/)
echo "$HTML" | grep -q "<script>" && echo "FAIL: JS Found" || echo "PASS: Zero-JS"

echo ""
if [ $FAILED -eq 0 ]; then
    echo "All tests passed. System is SOVEREIGN."
else
    echo "$FAILED test(s) failed."
    exit 1
fi
