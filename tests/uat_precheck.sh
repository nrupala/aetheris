#!/bin/bash
# Pre-requisites check before UAT
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "========================================="
echo "AETHERIS UAT PRE-REQUISITES CHECK"
echo "========================================="
echo ""

ERRORS=0

check() {
    local name=$1
    local cmd=$2

    echo -n "Checking $name... "
    if eval "$cmd" >/dev/null 2>&1; then
        echo "OK"
    else
        echo "MISSING"
        ((ERRORS++)) || true
    fi
}

# System requirements
echo "SYSTEM REQUIREMENTS:"
check "Docker" "command -v docker"
check "Docker Compose" "command -v docker || command -v docker-compose"
check "Curl" "command -v curl"
check "jq" "command -v jq"

# Directory structure
echo ""
echo "DIRECTORY STRUCTURE:"
DIRS=("core" "config" "scripts" "data" "vault" "tests")
for dir in "${DIRS[@]}"; do
    echo -n "  $PROJECT_ROOT/$dir... "
    if [ -d "$PROJECT_ROOT/$dir" ]; then
        echo "OK"
    else
        echo "MISSING"
        ((ERRORS++)) || true
    fi
done

# Key files
echo ""
echo "KEY FILES:"
FILES=(
    "$PROJECT_ROOT/core/Cargo.toml"
    "$PROJECT_ROOT/compose.yaml"
    "$PROJECT_ROOT/Dockerfile.core"
    "$PROJECT_ROOT/config/policy/policy.rego"
    "$PROJECT_ROOT/scripts/bootstrap.sh"
    "$PROJECT_ROOT/scripts/verification.sh"
)
for file in "${FILES[@]}"; do
    echo -n "  $file... "
    if [ -f "$file" ]; then
        echo "OK"
    else
        echo "MISSING"
        ((ERRORS++)) || true
    fi
done

# Scripts are executable
echo ""
echo "SCRIPT PERMISSIONS:"
SCRIPTS=(
    "$PROJECT_ROOT/scripts/bootstrap.sh"
    "$PROJECT_ROOT/scripts/verification.sh"
    "$PROJECT_ROOT/scripts/killswitch.sh"
    "$PROJECT_ROOT/scripts/vault_setup.sh"
)
for script in "${SCRIPTS[@]}"; do
    echo -n "  $script... "
    if [ -x "$script" ]; then
        echo "OK (executable)"
    elif [ -f "$script" ]; then
        echo "WARNING (not executable)"
    else
        echo "MISSING"
        ((ERRORS++)) || true
    fi
done

echo ""
echo "========================================="
if [ $ERRORS -eq 0 ]; then
    echo "PRE-REQUISITES: ALL OK"
    echo "Ready to run UAT tests"
    exit 0
else
    echo "PRE-REQUISITES: $ERRORS ERRORS FOUND"
    echo "Please fix errors before running UAT"
    exit 1
fi
