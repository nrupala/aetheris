#!/bin/bash
# UAT-01: Infrastructure Tests
set -e
FAILED=0
PASSED=0

echo "========================================="
echo "UAT-01: INFRASTRUCTURE TESTS"
echo "========================================="

# UAT-01.01: Docker Runtime
echo "[UAT-01.01] Testing Docker Runtime..."
if command -v docker >/dev/null 2>&1; then
    VERSION=$(docker version --format '{{.Server.Version}}' 2>/dev/null || echo "0")
    if [ "$VERSION" != "0" ]; then
        echo "  PASS: Docker version $VERSION"
        ((PASSED++))
    else
        echo "  FAIL: Cannot get Docker version"
        ((FAILED++))
    fi
else
    echo "  FAIL: Docker not installed"
    ((FAILED++))
fi

# UAT-01.02: Container Network
echo "[UAT-01.02] Testing Container Network..."
NETWORK=$(docker network ls --format '{{.Name}}' | grep aetheris_internal || echo "")
if [ -n "$NETWORK" ]; then
    echo "  PASS: Network aetheris_internal exists"
    ((PASSED++))
else
    echo "  SKIP: Network not created yet (run docker compose up)"
fi

# UAT-01.03: Volume Mounts
echo "[UAT-01.03] Testing Volume Mounts..."
MOUNT_OK=true
for dir in vault data/ollama data/chroma data/victoria config; do
    if [ ! -d "/opt/aetheris/$dir" ]; then
        MOUNT_OK=false
        echo "  FAIL: /opt/aetheris/$dir not found"
        ((FAILED++))
        break
    fi
done
if [ "$MOUNT_OK" = true ]; then
    echo "  PASS: All directories exist"
    ((PASSED++))
fi

# UAT-01.04: Rust Binary
echo "[UAT-01.04] Testing Rust Binary..."
if [ -f "/opt/aetheris/core/target/x86_64-unknown-linux-musl/release/aetheris-core" ]; then
    SIZE=$(stat -f%z /opt/aetheris/core/target/x86_64-unknown-linux-musl/release/aetheris-core 2>/dev/null || stat -c%s /opt/aetheris/core/target/x86_64-unknown-linux-musl/release/aetheris-core 2>/dev/null || echo "0")
    echo "  PASS: Binary exists, size: $SIZE bytes"
    ((PASSED++))
else
    echo "  FAIL: Binary not found"
    ((FAILED++))
fi

echo ""
echo "UAT-01 RESULTS: $PASSED passed, $FAILED failed"
[ $FAILED -eq 0 ] && exit 0 || exit 1
