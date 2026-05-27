#!/bin/bash
# Port Allocator — scans for available ports, exports env vars for Docker Compose
# Also writes JSON registry for service discovery.
# Usage: eval "$(bash scripts/port_allocator.sh)"
# Falls back through alternative ports if primary is taken.

set -euo pipefail

SERVICES=(
  "AETHERIS_CORE:8080:8080,8081,8082,8083"
  "OPA_GATEWAY:8181:8181,8182,8183,8184"
  "VICTORIA_METRICS:8428:8428,8429,8430,8431"
  "NGINX_HTTP:80:9080,9081,9082,9083"
  "NGINX_HTTPS:443:9443,9444,9445,9446"
  "LLMVM_DEV:8443:8443,8444,8445,8446"
  "LLMVM_ORCHESTRATOR:9090:9090,9091,9092,9093"
  "WIREGUARD:51820:51820,51821,51822,51823"
)

port_available() {
  local port=$1
  case "$(uname -s)" in
    Linux*)
      if command -v ss &>/dev/null; then
        ! ss -tlnp "sport = :$port" 2>/dev/null | grep -q LISTEN
      elif [ -f /proc/net/tcp ]; then
        local hex
        hex=$(printf "%04X" "$port" 2>/dev/null || printf "%x" "$port")
        ! grep -qi ":${hex} " /proc/net/tcp
      else
        ! timeout 1 bash -c "echo >/dev/tcp/localhost/$port" 2>/dev/null
      fi
      ;;
    Darwin*)
      ! lsof -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | grep -q LISTEN
      ;;
    MINGW*|MSYS*|CYGWIN*)
      netstat -an 2>/dev/null | grep -q ":$port " && return 1 || return 0
      ;;
    *)
      ! timeout 1 bash -c "echo >/dev/tcp/localhost/$port" 2>/dev/null
      ;;
  esac
}

ALLOCATED=()
FAILED_SERVICES=()

for entry in "${SERVICES[@]}"; do
  IFS=':' read -r var_name internal_port port_list <<< "$entry"
  IFS=',' read -ra ports <<< "$port_list"
  allocated=""
  for port in "${ports[@]}"; do
    if [ "$port" -eq "$internal_port" ] || port_available "$port"; then
      allocated=$port
      break
    fi
  done
  if [ -z "$allocated" ]; then
    echo "export ${var_name}_PORT=${ports[0]}"
    echo "export ${var_name}_FAILED=true" >&2
    echo "${var_name}: ALL PORTS BUSY (tried ${port_list})" >&2
    FAILED_SERVICES+=("$var_name")
    ALLOCATED+=("{\"name\":\"${var_name}\",\"internal_port\":${internal_port},\"external_port\":${ports[0]},\"protocol\":\"tcp\",\"status\":\"failed\"}")
  else
    echo "export ${var_name}_PORT=$allocated"
    echo "export ${var_name}_FAILED=false" >&2
    ALLOCATED+=("{\"name\":\"${var_name}\",\"internal_port\":${internal_port},\"external_port\":${allocated},\"protocol\":\"tcp\",\"status\":\"allocated\"}")
  fi
done

echo "export PORT_ALLOCATION_DONE=true"

# Build JSON array
JSON="{\"generated_at\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"services\":["
first=true
for entry in "${ALLOCATED[@]}"; do
  if [ "$first" = true ]; then
    first=false
  else
    JSON+=","
  fi
  JSON+="$entry"
done
JSON+="]}"

echo "$JSON" > "${PORT_REGISTRY_PATH:-config/port_registry.json}"
echo "Wrote port registry to ${PORT_REGISTRY_PATH:-config/port_registry.json}" >&2
