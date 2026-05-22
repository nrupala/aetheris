#!/bin/bash
# Aetheris AI Connection Manager
# Switches between local LMStudio and remote Oracle VM inference
# Used by Aetheris Core to determine optimal AI endpoint

set -euo pipefail

# ============================================
# CONFIGURATION
# ============================================
LOCAL_AI="${LOCAL_AI:-http://host.docker.internal:1234}"
REMOTE_AI="${REMOTE_AI:-https://ai.your-domain.com}"
REMOTE_API_KEY="${REMOTE_API_KEY:-}"
DEFAULT_MODEL="${DEFAULT_MODEL:-microsoft/phi-4-reasoning-plus}"
TIMEOUT="${AI_TIMEOUT:-10}"

# ============================================
# FUNCTIONS
# ============================================

check_local() {
    echo "Checking local LMStudio..."
    if curl -sf --max-time $TIMEOUT "$LOCAL_AI/v1/models" > /dev/null 2>&1; then
        local models=$(curl -sf --max-time $TIMEOUT "$LOCAL_AI/v1/models" 2>/dev/null)
        if echo "$models" | grep -q "$DEFAULT_MODEL"; then
            echo "LOCAL:available"
            return 0
        fi
        echo "LOCAL:model_not_loaded"
        return 1
    fi
    echo "LOCAL:unavailable"
    return 1
}

check_remote() {
    echo "Checking remote AI VM..."
    if curl -sf --max-time $TIMEOUT "$REMOTE_AI/v1/models" \
        -H "Authorization: Bearer $REMOTE_API_KEY" > /dev/null 2>&1; then
        echo "REMOTE:available"
        return 0
    fi
    echo "REMOTE:unavailable"
    return 1
}

# ============================================
# MAIN: Select best endpoint
# ============================================
select_endpoint() {
    local local_status=$(check_local)
    local remote_status=$(check_remote)

    echo ""
    echo "=== Aetheris AI Endpoint Selection ==="
    echo "Local:  $local_status"
    echo "Remote: $remote_status"
    echo "Model:  $DEFAULT_MODEL"
    echo ""

    # Priority: Local with model loaded > Remote > Local without model
    if [[ "$local_status" == "LOCAL:available" ]]; then
        echo "Selected: LOCAL ($LOCAL_AI)"
        echo "ENDPOINT=$LOCAL_AI"
        echo "API_KEY="
    elif [[ "$remote_status" == "REMOTE:available" ]]; then
        echo "Selected: REMOTE ($REMOTE_AI)"
        echo "ENDPOINT=$REMOTE_AI"
        echo "API_KEY=$REMOTE_API_KEY"
    elif [[ "$local_status" == "LOCAL:model_not_loaded" ]]; then
        # Local is up but model not loaded - use remote for heavy models
        if [[ "$remote_status" == "REMOTE:available" ]]; then
            echo "Selected: REMOTE (local up but model not loaded)"
            echo "ENDPOINT=$REMOTE_AI"
            echo "API_KEY=$REMOTE_API_KEY"
        else
            echo "Selected: LOCAL (remote down, will use whatever model is available)"
            echo "ENDPOINT=$LOCAL_AI"
            echo "API_KEY="
        fi
    else
        echo "ERROR: No AI endpoints available"
        echo "ENDPOINT=$LOCAL_AI"
        echo "API_KEY="
        return 1
    fi
}

# ============================================
# RUN
# ============================================
if [[ "${1:-}" == "test" ]]; then
    check_local
    check_remote
else
    select_endpoint
fi
