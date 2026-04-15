#!/usr/bin/env bash

DATASET="${1:-aetheris_vault/secure_data}"

echo "Available Recovery Points:"
zfs list -t snapshot -o name,creation -s creation | grep "$DATASET"

read -p "Enter snapshot name to rollback (or Ctrl+C to cancel): " SNAP_NAME

if [ -z "$SNAP_NAME" ]; then
    echo "No snapshot specified."
    exit 1
fi

echo "Unmounting dataset..."
zfs unmount "$DATASET" 2>/dev/null || true

echo "Rolling back to $SNAP_NAME..."
zfs rollback -r "$SNAP_NAME"

echo "Remounting dataset..."
zfs mount "$DATASET"

echo "Vault successfully reverted to $SNAP_NAME."
