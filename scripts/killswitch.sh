#!/usr/bin/env bash
set -e

VAULT_POOL="aetheris_vault"
KEY_FILE="/root/.aetheris_key"
DRY_RUN=false

[ "$1" == "--dry-run" ] && DRY_RUN=true

echo "AETHERIS EMERGENCY PURGE PROTOCOL"
[ "$DRY_RUN" == "true" ] && echo "(DRY RUN MODE - No changes will be made)" || echo "(WARNING: THIS IS IRREVERSIBLE)"

CMD_PREFIX=""
[ "$DRY_RUN" == "true" ] && CMD_PREFIX="echo Would execute:"

echo "Step 1: Stopping containers..."
$CMD_PREFIX docker compose down --volumes --timeout 0

echo "Step 2: Stopping WireGuard..."
$CMD_PREFIX systemctl stop wg-quick@wg0 2>/dev/null || true

echo "Step 3: Unmounting vault..."
$CMD_PREFIX zfs unmount -f "$VAULT_POOL/secure_data" 2>/dev/null || true

echo "Step 4: Unloading encryption keys..."
$CMD_PREFIX zfs unload-key "$VAULT_POOL/secure_data" 2>/dev/null || true

if [ "$DRY_RUN" == "false" ]; then
    echo "Step 5: Shredding local master key..."
    [ -f "$KEY_FILE" ] && shred -u -n 3 -z "$KEY_FILE"
    
    echo "Step 6: Clearing forensic traces..."
    echo > /var/log/auth.log 2>/dev/null || true
    echo > /var/log/kern.log 2>/dev/null || true
    history -c 2>/dev/null || true
    
    echo "DATA SECURED. Vault is now a black box."
    echo "Recovery requires offsite backup key."
else
    echo "Dry run complete. No changes made."
fi
