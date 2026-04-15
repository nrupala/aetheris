#!/usr/bin/env bash
set -e

USB_MOUNT="/mnt/aetheris_cold_storage"
POOL="aetheris_vault"

if mountpoint -q "$USB_MOUNT"; then
    echo "USB Cold Storage detected. Committing snapshots..."
    
    TIMESTAMP=$(date +%Y%m%d_%H%M%S)
    zfs send -Rwp "$POOL/secure_data@latest" > "$USB_MOUNT/aetheris_backup_$TIMESTAMP.zfs"
    
    echo "Offline Backup saved: aetheris_backup_$TIMESTAMP.zfs"
    sync && umount "$USB_MOUNT"
    echo "Safe to disconnect drive."
else
    echo "Offline drive not mounted at $USB_MOUNT"
    exit 1
fi
