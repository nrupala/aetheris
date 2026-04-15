#!/bin/bash
# Pre-requisites check before UAT
set -e

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
        ((ERRORS++))
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
    echo -n "  /opt/aetheris/$dir... "
    if [ -d "/opt/aetheris/$dir" ]; then
        echo "OK"
    else
        echo "MISSING"
        ((ERRORS++))
    fi
done

# Key files
echo ""
echo "KEY FILES:"
FILES=(
    "/opt/aetheris/core/Cargo.toml"
    "/opt/aetheris/compose.yaml"
    "/opt/aetheris/Dockerfile.core"
    "/opt/aetheris/config/policy/policy.rego"
    "/opt/aetheris/scripts/bootstrap.sh"
    "/opt/aetheris/scripts/verification.sh"
)
for file in "${FILES[@]}"; do
    echo -n "  $file... "
    if [ -f "$file" ]; then
        echo "OK"
    else
        echo "MISSING"
        ((ERRORS++))
    fi
done

# Scripts are executable
echo ""
echo "SCRIPT PERMISSIONS:"
SCRIPTS=(
    "/opt/aetheris/scripts/bootstrap.sh"
    "/opt/aetheris/scripts/verification.sh"
    "/opt/aetheris/scripts/killswitch.sh"
    "/opt/aetheris/scripts/vault_setup.sh"
)
for script in "${SCRIPTS[@]}"; do
    echo -n "  $script... "
    if [ -x "$script" ]; then
        echo "OK (executable)"
    elif [ -f "$script" ]; then
        echo "WARNING (not executable)"
    else
        echo "MISSING"
        ((ERRORS++))
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
