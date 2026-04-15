#!/bin/bash
# UAT-03: File Operations Tests
set -e
FAILED=0
PASSED=0

echo "========================================="
echo "UAT-03: FILE OPERATIONS TESTS"
echo "========================================="

TESTFILE="/tmp/uat_test_$(date +%s).txt"
CONTENT="Aetheris UAT Test File $(date)"

# UAT-03.01: File Upload
echo "[UAT-03.01] Testing File Upload..."
echo "$CONTENT" > "$TESTFILE"
RESP=$(curl -s -X POST -F "file=@$TESTFILE" http://localhost:8080/upload 2>/dev/null || echo '{"error":"failed"}')
if echo "$RESP" | grep -q '"status":"uploaded"'; then
    echo "  PASS: File uploaded successfully"
    ((PASSED++))
else
    echo "  FAIL: Upload failed (response: $RESP)"
    ((FAILED++))
fi

# UAT-03.02: File Download
echo "[UAT-03.02] Testing File Download..."
FILENAME=$(basename "$TESTFILE")
DOWNLOADED="/tmp/uat_download_$(date +%s).txt"
HTTP_CODE=$(curl -s -o "$DOWNLOADED" -w "%{http_code}" "http://localhost:8080/download/$FILENAME" 2>/dev/null || echo "000")
if [ "$HTTP_CODE" == "200" ]; then
    if diff "$TESTFILE" "$DOWNLOADED" >/dev/null 2>&1; then
        echo "  PASS: File downloaded with correct content"
        ((PASSED++))
    else
        echo "  FAIL: Downloaded content mismatch"
        ((FAILED++))
    fi
else
    echo "  FAIL: Download failed (HTTP $HTTP_CODE)"
    ((FAILED++))
fi

# UAT-03.03: Non-existent File
echo "[UAT-03.03] Testing Non-existent File..."
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/download/nonexistent_file_xyz.txt" 2>/dev/null || echo "000")
if [ "$HTTP_CODE" == "404" ]; then
    echo "  PASS: Non-existent file returns 404"
    ((PASSED++))
else
    echo "  FAIL: Expected 404, got $HTTP_CODE"
    ((FAILED++))
fi

# UAT-03.04: Large File Test
echo "[UAT-03.04] Testing Large File (1MB)..."
dd if=/dev/urandom of=/tmp/uat_large.bin bs=1M count=1 2>/dev/null
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST -F "file=@/tmp/uat_large.bin" http://localhost:8080/upload 2>/dev/null || echo "000")
if [ "$HTTP_CODE" == "200" ]; then
    echo "  PASS: Large file uploaded"
    ((PASSED++))
else
    echo "  FAIL: Large file upload failed (HTTP $HTTP_CODE)"
    ((FAILED++))
fi

# Cleanup
rm -f "$TESTFILE" "$DOWNLOADED" /tmp/uat_large.bin 2>/dev/null

echo ""
echo "UAT-03 RESULTS: $PASSED passed, $FAILED failed"
[ $FAILED -eq 0 ] && exit 0 || exit 1
