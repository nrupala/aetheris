#!/usr/bin/env bash
set -e

POOL_NAME="aetheris_vault"
DATASET_NAME="${POOL_NAME}/secure_data"
KEY_FILE="/root/.aetheris_key"

echo "Provisioning Zero-Trust Encrypted Persistent Vault..."

if [ ! -f "$KEY_FILE" ]; then
    echo "Generating master vault key..."
    openssl rand -hex 32 > "$KEY_FILE"
    chmod 600 "$KEY_FILE"
fi

DISK_ID="${1:-YOUR_DISK_ID_HERE}"

if ! zpool list "$POOL_NAME" > /dev/null 2>&1; then
    echo "Creating ZFS pool..."
    zpool create -f "$POOL_NAME" "/dev/disk/by-id/${DISK_ID}"
fi

if ! zfs list "$DATASET_NAME" > /dev/null 2>&1; then
    echo "Creating encrypted dataset..."
    zfs create \
        -o encryption=on \
        -o keyformat=hex \
        -o keylocation="file://${KEY_FILE}" \
        -o compression=lz4 \
        -o xattr=sa \
        -o acltype=posixacl \
        "$DATASET_NAME"
    echo "Vault created and encrypted."
else
    echo "Loading existing vault keys..."
    zfs load-key "$DATASET_NAME"
    zfs mount "$DATASET_NAME"
fi

mkdir -p /opt/aetheris/vault
mount_point=$(zfs get -H -o value mountpoint "$DATASET_NAME")
[ "$mount_point" != "/opt/aetheris/vault" ] && zfs set mountpoint=/opt/aetheris/vault "$DATASET_NAME"

echo "Vault is online and persistent."
