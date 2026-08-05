#!/usr/bin/env bash
# Refresh the Cloudflare Access JWKS, keeping the last-good file on any failure.
# Runs hourly via cf-access-jwks.timer; also called from install-native.sh.
set -euo pipefail

TEAM="${CF_ACCESS_TEAM_DOMAIN:-https://nrupal.cloudflareaccess.com}"
DST="${CF_ACCESS_JWKS_PATH:-/etc/aetheris/cf_access_jwks.json}"

TMP="$(mktemp)"
# -f: fail on HTTP errors; -sSL: silent but show errors. On success only, we
# atomically replace the destination (install keeps 0644 + root owner), so a
# transient/network failure leaves the previous good file untouched.
if curl -fsSL "${TEAM}/cdn-cgi/access/certs" -o "${TMP}"; then
    install -m 0644 "${TMP}" "${DST}"
fi
rm -f "${TMP}"
