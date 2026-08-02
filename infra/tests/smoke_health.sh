#!/usr/bin/env bash
# Aetheris full-stack health smoke test.
# Tracked in infra/tests/smoke_health.sh — run on the host with:
#   bash /opt/aetheris/infra/tests/smoke_health.sh
set -u

BASE_BEE="http://127.0.0.1:8800"
BASE_MGMT="http://127.0.0.1:9090"
BASE_CS="http://127.0.0.1:8088"
BASE_LLM="http://127.0.0.1:11434"
BASE_CORE="http://127.0.0.1:8080"

pass=0
fail=0

check() {
    local name="$1"; shift
    local expect="$1"; shift
    local out
    out=$("$@" 2>&1) || true
    if echo "$out" | grep -q "$expect"; then
        echo "PASS  $name"
        pass=$((pass+1))
    else
        echo "FAIL  $name  (expected '$expect', got: $out)"
        fail=$((fail+1))
    fi
}

echo "== systemd services =="
for svc in ollama cloudflared bee code-server aetheris-mgmt; do
    check "systemd:$svc active" "active" bash -c "systemctl is-active $svc"
done

echo "== docker containers =="
for c in aetheris_core llmvm_nginx; do
    check "docker:$c running" "true" bash -c "docker inspect -f '{{.State.Running}}' $c"
    check "docker:$c restart-policy" "unless-stopped" bash -c "docker inspect -f '{{.HostConfig.RestartPolicy.Name}}' $c"
done

echo "== HTTP endpoints =="
check "bee health" "ok" curl -s -m 10 "$BASE_BEE/api/health"
check "mgmt health" "200" bash -c "curl -s -o /dev/null -w '%{http_code}' -m 10 $BASE_MGMT/health"
check "code-server healthz" "200" bash -c "curl -s -o /dev/null -w '%{http_code}' -m 10 $BASE_CS/healthz"
check "ollama tags" "models" curl -s -m 10 "$BASE_LLM/api/tags"
check "core health" "ai_connected" curl -s -m 10 "$BASE_CORE/health"

echo "== core AI bridge =="
EMBED_JSON=$(python3 -c 'import json; print(json.dumps({"text":"health check"}))')
FUSION_JSON=$(python3 -c 'import json; print(json.dumps({"query":"Reply with exactly: OK"}))')
check "core embed resolves" "768" bash -c "curl -s -m 30 $BASE_CORE/bridge/ai/embed -H 'Content-Type: application/json' -d '$EMBED_JSON' | python3 -c 'import sys,json; d=json.load(sys.stdin); print(len(d[\"embedding\"]))'"
check "core fusion generation" "OK" bash -c "curl -s -m 180 $BASE_CORE/fusion/query -H 'Content-Type: application/json' -d '$FUSION_JSON' | python3 -c 'import sys,json; print(json.load(sys.stdin)[\"answer\"])'"

echo ""
echo "RESULT: $pass passed, $fail failed"
if [ "$fail" -gt 0 ]; then exit 1; fi
exit 0
