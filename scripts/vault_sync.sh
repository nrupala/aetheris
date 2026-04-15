#!/usr/bin/env bash
set -e

DATASET="aetheris_vault/secure_data"
REMOTE_USER="${REMOTE_USER:-backup_agent}"
REMOTE_HOST="${REMOTE_HOST:-offsite.node.local}"
REMOTE_DATASET="backup_pool/aetheris_mirror"

echo "Initiating Offsite Replication..."

LATEST_SNAP=$(zfs list -t snapshot -o name -s creation | grep "$DATASET" | tail -n 1)

if [ -z "$LATEST_SNAP" ]; then
    echo "No snapshots found."
    exit 1
fi

LAST_SENT=$(ssh $REMOTE_USER@$REMOTE_HOST "zfs list -t snapshot -o name -s creation | grep $REMOTE_DATASET | tail -n 1" 2>/dev/null | awk -F'@' '{print $2}')

if [ -z "$LAST_SENT" ]; then
    echo "Initializing full recursive encrypted transfer..."
    zfs send -Rwp "$LATEST_SNAP" | ssh $REMOTE_USER@$REMOTE_HOST "zfs receive -F $REMOTE_DATASET"
else
    echo "Sending incremental delta: $LAST_SENT -> $LATEST_SNAP"
    zfs send -Rwp -i "$DATASET@$LAST_SENT" "$LATEST_SNAP" | ssh $REMOTE_USER@$REMOTE_HOST "zfs receive -F $REMOTE_DATASET"
fi

echo "Offsite Replication Complete: $LATEST_SNAP"
