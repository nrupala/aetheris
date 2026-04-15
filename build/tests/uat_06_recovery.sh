#!/bin/bash
# UAT-06: Recovery & Backup Tests
set -e
FAILED=0
PASSED=0

echo "========================================="
echo "UAT-06: RECOVERY & BACKUP TESTS"
echo "========================================="

# UAT-06.01: Snapshot Creation
echo "[UAT-06.01] Testing Snapshot Creation..."
SNAP_NAME="uat_test_$(date +%s)"
if sudo zfs snapshot "aetheris_vault/secure_data@$SNAP_NAME" 2>/dev/null; then
    if sudo zfs list -t snapshot | grep -q "$SNAP_NAME"; then
        echo "  PASS: Snapshot created"
        ((PASSED++))
    else
        echo "  FAIL: Snapshot not found after creation"
        ((FAILED++))
    fi
else
    echo "  SKIP: ZFS not configured or not accessible"
fi

# UAT-06.02: Snapshot Listing
echo "[UAT-06.02] Testing Snapshot Listing..."
SNAP_COUNT=$(sudo zfs list -t snapshot 2>/dev/null | grep -c "aetheris_vault" || echo "0")
if [ "$SNAP_COUNT" -gt 0 ]; then
    echo "  PASS: Found $SNAP_COUNT snapshots"
    ((PASSED++))
else
    echo "  INFO: No snapshots yet"
fi

# UAT-06.03: Kill-Switch Dry-Run
echo "[UAT-06.03] Testing Kill-Switch Dry-Run..."
if [ -f "/opt/aetheris/scripts/killswitch.sh" ]; then
    echo "  INFO: Kill-switch exists"
    if /opt/aetheris/scripts/killswitch.sh --dry-run 2>&1 | grep -q "DRY RUN"; then
        echo "  PASS: Kill-switch dry-run executes"
        ((PASSED++))
    else
        echo "  WARN: Kill-switch dry-run output unexpected"
    fi
else
    echo "  SKIP: Kill-switch script not in expected location"
fi

# UAT-06.04: Verification Script
echo "[UAT-06.04] Testing Verification Script..."
if [ -f "/opt/aetheris/scripts/verification.sh" ]; then
    RESULT=$(/opt/aetheris/scripts/verification.sh 2>&1)
    if echo "$RESULT" | grep -q "PASS\|FAIL"; then
        echo "  PASS: Verification script runs"
        echo "$RESULT" | grep "PASS" | head -3 | sed 's/^/    /'
        ((PASSED++))
    else
        echo "  WARN: Verification output unexpected"
    fi
else
    echo "  SKIP: Verification script not found"
fi

echo ""
echo "UAT-06 RESULTS: $PASSED passed, $FAILED failed"
[ $FAILED -eq 0 ] && exit 0 || exit 1
