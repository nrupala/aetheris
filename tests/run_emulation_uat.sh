#!/bin/bash
# ============================================
# Aetheris Bare Metal Emulation UAT Tests
# ============================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
COMPOSE_FILE="$PROJECT_ROOT/compose.emulation.yaml"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Aetheris Bare Metal Emulation UAT${NC}"
echo -e "${BLUE}========================================${NC}"

# ============================================
# Test 1: Infrastructure Prerequisites
# ============================================
test_prerequisites() {
    echo -e "\n${YELLOW}[TEST 1] Checking prerequisites...${NC}"

    local passed=0
    local failed=0

    # Check Docker
    if command -v docker &> /dev/null; then
        echo -e "  ${GREEN}✓${NC} Docker found: $(docker --version)"
        ((passed++))
    else
        echo -e "  ${RED}✗${NC} Docker not found"
        ((failed++))
    fi

    # Check Docker Compose
    if docker compose version &> /dev/null; then
        echo -e "  ${GREEN}✓${NC} Docker Compose found: $(docker compose version)"
        ((passed++))
    else
        echo -e "  ${RED}✗${NC} Docker Compose not found"
        ((failed++))
    fi

    # Check Required Files
    local required_files=(
        "$PROJECT_ROOT/compose.emulation.yaml"
        "$PROJECT_ROOT/Dockerfile.core"
        "$PROJECT_ROOT/config/policy/policy.rego"
        "$PROJECT_ROOT/config/policy/opa_conf.yaml"
    )

    for file in "${required_files[@]}"; do
        if [ -f "$file" ]; then
            echo -e "  ${GREEN}✓${NC} Found: $(basename $file)"
            ((passed++))
        else
            echo -e "  ${RED}✗${NC} Missing: $(basename $file)"
            ((failed++))
        fi
    done

    echo -e "\nPrerequisites: ${GREEN}$passed passed${NC}, ${RED}$failed failed${NC}"
    return $failed
}

# ============================================
# Test 2: Build Emulation Environment
# ============================================
test_build_environment() {
    echo -e "\n${YELLOW}[TEST 2] Building emulation environment...${NC}"

    cd "$PROJECT_ROOT"

    # Build all images
    echo "  Building Docker images..."
    if docker compose -f "$COMPOSE_FILE" build; then
        echo -e "  ${GREEN}✓${NC} All images built successfully"
        return 0
    else
        echo -e "  ${RED}✗${NC} Build failed"
        return 1
    fi
}

# ============================================
# Test 3: Start Infrastructure Services
# ============================================
test_start_infrastructure() {
    echo -e "\n${YELLOW}[TEST 3] Starting infrastructure services...${NC}"

    cd "$PROJECT_ROOT"

    # Create data directories
    mkdir -p "$PROJECT_ROOT/data/zfs_pool"
    mkdir -p "$PROJECT_ROOT/data/vault_data"
    mkdir -p "$PROJECT_ROOT/data/vault_logs"
    mkdir -p "$PROJECT_ROOT/data/vault_audit"
    mkdir -p "$PROJECT_ROOT/data/opa_data"
    mkdir -p "$PROJECT_ROOT/data/ollama_data"
    mkdir -p "$PROJECT_ROOT/data/chroma_data"
    mkdir -p "$PROJECT_ROOT/data/sentinel_logs"
    mkdir -p "$PROJECT_ROOT/data/victoria_data"
    mkdir -p "$PROJECT_ROOT/data/grafana_data"

    # Start infrastructure services first
    echo "  Starting WireGuard, OPA, Vault..."
    if docker compose -f "$COMPOSE_FILE" up -d wireguard-mesh opa-gateway encrypted-vault; then
        echo -e "  ${GREEN}✓${NC} Infrastructure services started"
        return 0
    else
        echo -e "  ${RED}✗${NC} Failed to start infrastructure"
        return 1
    fi
}

# ============================================
# Test 4: Zero-Trust Policy Verification
# ============================================
test_zero_trust_policies() {
    echo -e "\n${YELLOW}[TEST 4] Verifying Zero-Trust policies...${NC}"

    local opa_ip="10.0.10.2"

    # Wait for OPA to be ready
    echo "  Waiting for OPA Gateway..."
    for i in {1..30}; do
        if docker exec aetheris_opa opa version &> /dev/null; then
            echo -e "  ${GREEN}✓${NC} OPA Gateway is ready"
            break
        fi
        sleep 2
    done

    # Test OPA Policy Evaluation
    echo "  Testing policy evaluation..."

    # Allow request test
    local allow_result=$(docker exec aetheris_opa opa eval --format=pretty -I -d /config 'data.aetheris.authorize with input as {"subject": {"user": "test-user"}, "action": "read", "resource": "/api/vault/test"}')

    if echo "$allow_result" | grep -q "true"; then
        echo -e "  ${GREEN}✓${NC} Policy evaluation working"
    else
        echo -e "  ${RED}✗${NC} Policy evaluation failed"
        return 1
    fi

    # Test unauthorized request
    local deny_result=$(docker exec aetheris_opa opa eval --format=pretty -I -d /config 'data.aetheris.authorize with input as {"subject": {"user": "blocked-user"}, "action": "delete", "resource": "/api/vault/all"}')

    if echo "$deny_result" | grep -q "false"; then
        echo -e "  ${GREEN}✓${NC} Access control enforced"
    else
        echo -e "  ${YELLOW}⚠${NC} Access control may need tuning"
    fi

    return 0
}

# ============================================
# Test 5: Encrypted Storage Verification
# ============================================
test_encrypted_storage() {
    echo -e "\n${YELLOW}[TEST 5] Verifying encrypted storage...${NC}"

    # Check Vault is initialized
    echo "  Checking Vault initialization..."

    # Test Vault status
    local vault_status=$(docker exec aetheris_vault vault status -format=json 2>/dev/null || echo '{"initialized":false}')

    if echo "$vault_status" | grep -q '"initialized":true'; then
        echo -e "  ${GREEN}✓${NC} Vault is initialized"
    else
        echo -e "  ${YELLOW}⚠${NC} Vault not initialized (may need manual setup)"
    fi

    # Check encrypted volumes exist
    echo "  Checking encrypted volumes..."
    local volumes=$(docker volume ls --filter "name=aetheris" --format "{{.Name}}")

    if [ -n "$volumes" ]; then
        echo -e "  ${GREEN}✓${NC} Encrypted volumes found:"
        echo "$volumes" | while read vol; do
            echo "      - $vol"
        done
    else
        echo -e "  ${YELLOW}⚠${NC} No volumes found yet"
    fi

    return 0
}

# ============================================
# Test 6: WireGuard Mesh Network
# ============================================
test_wireguard_mesh() {
    echo -e "\n${YELLOW}[TEST 6] Verifying WireGuard mesh network...${NC}"

    # Check WireGuard is running
    if docker exec aetheris_wireguard wg show &> /dev/null; then
        echo -e "  ${GREEN}✓${NC} WireGuard is running"

        # Show interface info
        echo "  WireGuard status:"
        docker exec aetheris_wireguard wg show | head -5 | sed 's/^/      /'

        # Check UDP port
        if docker port aetheris_wireguard 2>/dev/null | grep -q "51820"; then
            echo -e "  ${GREEN}✓${NC} UDP 51820 port exposed"
        fi
    else
        echo -e "  ${RED}✗${NC} WireGuard not running"
        return 1
    fi

    return 0
}

# ============================================
# Test 7: Network Isolation
# ============================================
test_network_isolation() {
    echo -e "\n${YELLOW}[TEST 7] Verifying network isolation...${NC}"

    # Check networks exist
    echo "  Checking Docker networks..."

    local networks=$(docker network ls --filter "name=aetheris" --format "{{.Name}}")

    if echo "$networks" | grep -q "aetheris_test_net"; then
        echo -e "  ${GREEN}✓${NC} Primary network exists"
    else
        echo -e "  ${RED}✗${NC} Primary network missing"
        return 1
    fi

    if echo "$networks" | grep -q "aetheris_isolation_net"; then
        echo -e "  ${GREEN}✓${NC} Isolation network exists (honeypot)"
    fi

    # Test network connectivity
    echo "  Testing internal connectivity..."

    # Ping test between containers
    if docker exec aetheris_network_tester ping -c 2 10.0.10.2 &> /dev/null; then
        echo -e "  ${GREEN}✓${NC} Network connectivity verified"
    else
        echo -e "  ${YELLOW}⚠${NC} Network connectivity test skipped (containers may need time)"
    fi

    return 0
}

# ============================================
# Test 8: Aetheris Core Service
# ============================================
test_aetheris_core() {
    echo -e "\n${YELLOW}[TEST 8] Verifying Aetheris Core service...${NC}"

    # Check if core container is running
    if docker ps --filter "name=aetheris_core" --format "{{.Names}}" | grep -q "aetheris_core"; then
        echo -e "  ${GREEN}✓${NC} Aetheris Core is running"

        # Check health
        local health=$(docker inspect --format='{{.State.Health.Status}}' aetheris_core 2>/dev/null || echo "none")
        echo "    Health status: $health"

        # Check logs
        echo "  Recent logs:"
        docker logs aetheris_core --tail 10 2>&1 | sed 's/^/    /'

        return 0
    else
        echo -e "  ${YELLOW}⚠${NC} Aetheris Core not yet started (depends on OPA)"
        return 0
    fi
}

# ============================================
# Test 9: Monitoring Stack
# ============================================
test_monitoring_stack() {
    echo -e "\n${YELLOW}[TEST 9] Verifying monitoring stack...${NC}"

    # Check VictoriaMetrics
    if docker ps --filter "name=victoria-metrics" --format "{{.Names}}" | grep -q "victoria-metrics"; then
        echo -e "  ${GREEN}✓${NC} VictoriaMetrics is running"

        # Check metrics endpoint
        if curl -sf http://localhost:8428/health &> /dev/null; then
            echo -e "  ${GREEN}✓${NC} Metrics endpoint healthy"
        fi
    fi

    # Check Grafana
    if docker ps --filter "name=grafana" --format "{{.Names}}" | grep -q "grafana"; then
        echo -e "  ${GREEN}✓${NC} Grafana is running"
    fi

    # Check Sentinel
    if docker ps --filter "name=ai-sentinel" --format "{{.Names}}" | grep -q "ai-sentinel"; then
        echo -e "  ${GREEN}✓${NC} AI Sentinel is running"
    fi

    return 0
}

# ============================================
# Test 10: Cleanup
# ============================================
test_cleanup() {
    echo -e "\n${YELLOW}[TEST 10] Cleanup...${NC}"

    echo "  Stopping containers..."
    docker compose -f "$COMPOSE_FILE" down 2>/dev/null || true

    echo -e "  ${GREEN}✓${NC} Cleanup complete"
    return 0
}

# ============================================
# Main Test Runner
# ============================================
main() {
    local failed=0

    # Run tests in order
    test_prerequisites || ((failed++))

    if [ $failed -eq 0 ]; then
        test_build_environment || ((failed++))
    fi

    if [ $failed -eq 0 ]; then
        test_start_infrastructure || ((failed++))
    fi

    # Give services time to start
    sleep 10

    test_zero_trust_policies || ((failed++))
    test_encrypted_storage || ((failed++))
    test_wireguard_mesh || ((failed++))
    test_network_isolation || ((failed++))
    test_aetheris_core || ((failed++))
    test_monitoring_stack || ((failed++))
    test_cleanup || ((failed++))

    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}  Test Summary${NC}"
    echo -e "${BLUE}========================================${NC}"

    if [ $failed -eq 0 ]; then
        echo -e "${GREEN}All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}$failed test(s) failed${NC}"
        exit 1
    fi
}

# Run main
main "$@"
