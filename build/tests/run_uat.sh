#!/bin/bash
# UAT Master Runner - Execute all UAT suites
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=============================================="
echo "AETHERIS UAT MASTER TEST RUNNER"
echo "=============================================="
echo "Start Time: $(date)"
echo ""

TOTAL_PASS=0
TOTAL_FAIL=0
RESULTS_DIR="./uat_results"
mkdir -p "$RESULTS_DIR"

run_uat() {
    local uat_num=$1
    local uat_name=$2
    local script=$3
    
    echo ""
    echo "----------------------------------------------"
    echo "EXECUTING: $uat_num - $uat_name"
    echo "----------------------------------------------"
    
    START=$(date +%s)
    if bash "$script" 2>&1 | tee "$RESULTS_DIR/${uat_num}.log"; then
        echo "[$uat_num] RESULT: PASS"
        echo "$uat_num: PASS" >> "$RESULTS_DIR/summary.txt"
        ((TOTAL_PASS++))
    else
        echo "[$uat_num] RESULT: FAIL"
        echo "$uat_num: FAIL" >> "$RESULTS_DIR/summary.txt"
        ((TOTAL_FAIL++))
    fi
    END=$(date +%s)
    echo "Duration: $((END-START)) seconds"
}

# Initialize summary
echo "AETHERIS UAT SUMMARY" > "$RESULTS_DIR/summary.txt"
echo "Run Date: $(date)" >> "$RESULTS_DIR/summary.txt"
echo "" >> "$RESULTS_DIR/summary.txt"

# Execute all UAT suites
if [ -f "./uat_01_infrastructure.sh" ]; then
    run_uat "UAT-01" "Infrastructure" "./uat_01_infrastructure.sh"
fi

if [ -f "./uat_02_security.sh" ]; then
    run_uat "UAT-02" "Security" "./uat_02_security.sh"
fi

if [ -f "./uat_03_file_operations.sh" ]; then
    run_uat "UAT-03" "File Operations" "./uat_03_file_operations.sh"
fi

if [ -f "./uat_04_ai_semantic.sh" ]; then
    run_uat "UAT-04" "AI & Semantic" "./uat_04_ai_semantic.sh"
fi

if [ -f "./uat_05_network.sh" ]; then
    run_uat "UAT-05" "Network" "./uat_05_network.sh"
fi

if [ -f "./uat_06_recovery.sh" ]; then
    run_uat "UAT-06" "Recovery" "./uat_06_recovery.sh"
fi

# Final Report
echo ""
echo "=============================================="
echo "UAT EXECUTION COMPLETE"
echo "=============================================="
echo "End Time: $(date)"
echo ""
echo "RESULTS:"
cat "$RESULTS_DIR/summary.txt"
echo ""
echo "Total PASSED: $TOTAL_PASS"
echo "Total FAILED: $TOTAL_FAIL"
echo "Total Tests:  $((TOTAL_PASS+TOTAL_FAIL))"
echo ""

if [ $TOTAL_FAIL -eq 0 ]; then
    echo "STATUS: ALL UAT TESTS PASSED"
    echo "System is PRODUCTION READY"
    exit 0
else
    echo "STATUS: SOME UAT TESTS FAILED"
    echo "System requires fixes before production"
    exit 1
fi
