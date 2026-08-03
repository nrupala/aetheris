#!/usr/bin/env bash
# install-native.sh - install Aetheris Core as a native systemd service (no Docker).
#
# Idempotent: safe to re-run. It builds the static musl binary (as Dockerfile.core
# used to), installs it and the systemd unit, creates the dedicated service user, and
# enables the service. Run as root on the target host AFTER `git pull` on this repo:
#
#     sudo scripts/install-native.sh
#
# Nothing here touches Docker/compose/nginx - those are retired by the native cutover.
set -euo pipefail

# --- Config (override via environment if needed) ---
SERVICE_USER="${SERVICE_USER:-aetheris}"
SERVICE_GROUP="${SERVICE_GROUP:-aetheris}"
INSTALL_DIR="${INSTALL_DIR:-/opt/aetheris/bin}"
DATA_DIR="${DATA_DIR:-/data}"
ETC_DIR="${ETC_DIR:-/etc/aetheris}"
UNIT_DST="/etc/systemd/system/aetheris-core.service"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

log() { printf '==> %s\n' "$*"; }

if [[ "${EUID}" -ne 0 ]]; then
  echo "This installer must run as root (use sudo)." >&2
  exit 1
fi

# --- 1. Build the release binary (prefer the static musl build, as Dockerfile.core did) ---
ARCH="$(uname -m)"
case "${ARCH}" in
  x86_64)  TARGET="x86_64-unknown-linux-musl" ;;
  aarch64) TARGET="aarch64-unknown-linux-musl" ;;
  *) echo "Unsupported arch: ${ARCH}" >&2; exit 1 ;;
esac

log "Building aetheris-core for ${TARGET} (static musl)"
if command -v rustup >/dev/null 2>&1; then
  rustup target add "${TARGET}" >/dev/null 2>&1 || true
fi
( cd "${REPO_ROOT}/core" && cargo build --release --target "${TARGET}" )
BIN_SRC="${REPO_ROOT}/core/target/${TARGET}/release/aetheris-core"

if [[ ! -x "${BIN_SRC}" ]]; then
  # Fall back to the host-native (gnu) build if the musl target is unavailable.
  log "musl build not found; falling back to host-native build"
  ( cd "${REPO_ROOT}/core" && cargo build --release )
  BIN_SRC="${REPO_ROOT}/core/target/release/aetheris-core"
fi

# --- 2. Service user/group (installer creates it; the unit runs as it) ---
if ! getent group "${SERVICE_GROUP}" >/dev/null 2>&1; then
  log "Creating group ${SERVICE_GROUP}"
  groupadd --system "${SERVICE_GROUP}"
fi
if ! id -u "${SERVICE_USER}" >/dev/null 2>&1; then
  log "Creating system user ${SERVICE_USER}"
  useradd --system --gid "${SERVICE_GROUP}" --home-dir "${DATA_DIR}" \
          --shell /usr/sbin/nologin "${SERVICE_USER}"
fi

# --- 3. Install the binary ---
log "Installing binary to ${INSTALL_DIR}/aetheris-core"
install -d -m 0755 "${INSTALL_DIR}"
install -m 0755 "${BIN_SRC}" "${INSTALL_DIR}/aetheris-core"

# --- 4. Data dir (vault + WAL; matches the unit's ReadWritePaths=/data) ---
# /data is 0755 (world-traversable): Ollama's model store may live under
# /data/ollama-models and the ollama user must be able to traverse /data.
# Only the vault (0750, aetheris-only) is private.
install -d -o "${SERVICE_USER}" -g "${SERVICE_GROUP}" -m 0755 "${DATA_DIR}"
install -d -o "${SERVICE_USER}" -g "${SERVICE_GROUP}" -m 0750 "${DATA_DIR}/vault"

# --- 5. Environment file (from the example if absent; never overwrite a live one) ---
install -d -m 0755 "${ETC_DIR}"
if [[ ! -f "${ETC_DIR}/core.env" ]]; then
  log "Installing ${ETC_DIR}/core.env from config/core.env.example"
  install -m 0640 -g "${SERVICE_GROUP}" \
    "${REPO_ROOT}/config/core.env.example" "${ETC_DIR}/core.env"
else
  log "${ETC_DIR}/core.env already exists - leaving it untouched"
fi

# --- 5b. OPA policy engine (native, loopback-only) ---
# Installs the static OPA v1 binary, deploys the versioned policy, and enables
# opa.service. Loopback only (127.0.0.1:8181) - never expose 8181 via tunnel.
OPA_VERSION="${OPA_VERSION:-1.1.0}"
OPA_BIN="/usr/local/bin/opa"
OPA_POLICY_DIR="${ETC_DIR}/policy"
OPA_UNIT_DST="/etc/systemd/system/opa.service"

if [[ "${ARCH}" == "aarch64" ]]; then
  OPA_RELEASE_ARCH="arm64"
else
  OPA_RELEASE_ARCH="amd64"
fi

log "Installing OPA v${OPA_VERSION} (native, loopback-only, ${OPA_RELEASE_ARCH})"
if [[ ! -x "${OPA_BIN}" ]] || ! "${OPA_BIN}" version 2>/dev/null | grep -q "${OPA_VERSION}"; then
  log "Downloading OPA v${OPA_VERSION} static from GitHub releases"
  curl -fsSL -o /tmp/opa.aetheris \
    "https://github.com/open-policy-agent/opa/releases/download/v${OPA_VERSION}/opa_linux_${OPA_RELEASE_ARCH}_static"
  install -m 0755 /tmp/opa.aetheris "${OPA_BIN}"
  rm -f /tmp/opa.aetheris
fi
"${OPA_BIN}" version

log "Deploying OPA policy to ${OPA_POLICY_DIR}"
install -d -m 0755 "${OPA_POLICY_DIR}"
install -m 0644 "${REPO_ROOT}/config/policy/aetheris.authz.rego" "${OPA_POLICY_DIR}/"

log "Installing opa.service unit to ${OPA_UNIT_DST}"
install -m 0644 "${REPO_ROOT}/infra/systemd/opa.service" "${OPA_UNIT_DST}"
log "Reloading systemd and enabling opa"
systemctl daemon-reload
systemctl enable --now opa.service

# --- 6. systemd unit ---
log "Installing systemd unit to ${UNIT_DST}"
install -m 0644 "${REPO_ROOT}/infra/systemd/aetheris-core.service" "${UNIT_DST}"

# --- 7. Enable + (re)start ---
log "Reloading systemd and enabling aetheris-core"
systemctl daemon-reload
systemctl enable --now aetheris-core.service

log "Done. Check status with: systemctl status aetheris-core"

# ---------------------------------------------------------------------------
# cloudflared ingress (configure once on the host; NOT done by this script):
#
#   # /etc/cloudflared/config.yml
#   tunnel: <your-tunnel-id>
#   credentials-file: /etc/cloudflared/<your-tunnel-id>.json
#   ingress:
#     - hostname: core.nrupalakolkar.com
#       service: http://127.0.0.1:8080
#     - service: http_status:404
#
#   cloudflared tunnel route dns <tunnel-name> core.nrupalakolkar.com
#   systemctl restart cloudflared
#
# Gate core.nrupalakolkar.com behind Cloudflare Access (see docs/DEPLOY_NATIVE.md).
# Ollama stays loopback-only at 127.0.0.1:11434 - never add it to the ingress.
# ---------------------------------------------------------------------------
