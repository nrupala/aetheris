#!/bin/sh
# assert-writable.sh - fail loudly at service start if the target dir is not
# actually writable, so a read-only (EROFS) regression of the service's
# ReadWritePaths cannot pass silently.
#
# Why a real write and not `test -w`: when a filesystem is remounted read-only
# (EROFS), a directory can keep its writable permission bit yet reject every
# write. We prove writability by creating and removing a probe file instead.
#
# Wired into aetheris-core.service as the FIRST ExecStartPre:
#     ExecStartPre=/opt/aetheris/bin/assert-writable.sh /data
# It runs inside the unit sandbox as User=aetheris, so it checks exactly the
# write access the service itself has to its ReadWritePaths.
set -eu

dir="${1:-/data}"

if [ ! -d "$dir" ]; then
    echo "FATAL: $dir does not exist - refusing to start aetheris-core" >&2
    exit 1
fi

probe="$dir/.aetheris-write-probe.$$"
if ! ( : > "$probe" ) 2>/dev/null; then
    echo "FATAL: $dir is not writable (EROFS / read-only filesystem?) - refusing to start aetheris-core" >&2
    exit 1
fi
rm -f "$probe"
exit 0
