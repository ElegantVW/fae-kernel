#!/bin/sh
# Audit the KINDLING pen against the tree. Skip if the pen is out.
# Fail if the pen is present and the bytes (or sudo) lie.
set -eu
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
DEV=/dev/disk/by-label/KINDLING
EFI_SRC=kernel/BOOTX64.EFI
CAIRN_SRC=spark/cairn.bin

if [ ! -e "$DEV" ]; then
    echo "skip stick (no KINDLING)"
    exit 0
fi
if [ ! -f "$EFI_SRC" ] || [ ! -f "$CAIRN_SRC" ]; then
    echo "FAIL stick (missing $EFI_SRC or $CAIRN_SRC)"
    exit 1
fi
if ! sudo -n true >/dev/null 2>&1; then
    echo "skip stick (need sudo; run sudo -v in a tty)"
    md5sum "$EFI_SRC" "$CAIRN_SRC"
    exit 0
fi
MNT=$(mktemp -d /tmp/kindling-stick.XXXXXX)
cleanup() {
    sudo -n umount "$MNT" >/dev/null 2>&1 || true
    rmdir "$MNT" >/dev/null 2>&1 || true
}
trap cleanup EXIT
sudo -n mount -o ro,nosuid,nodev "$DEV" "$MNT"
got_efi="$MNT/EFI/BOOT/BOOTX64.EFI"
got_cairn="$MNT/EFI/BOOT/CAIRN"
if [ ! -f "$got_efi" ] || [ ! -f "$got_cairn" ]; then
    echo "FAIL stick (missing EFI/BOOT/BOOTX64.EFI or CAIRN)"
    exit 1
fi
cmp -s "$EFI_SRC" "$got_efi" || {
    echo "FAIL stick (BOOTX64.EFI differs from tree)"
    md5sum "$EFI_SRC" "$got_efi"
    exit 1
}
cmp -s "$CAIRN_SRC" "$got_cairn" || {
    echo "FAIL stick (CAIRN differs from tree)"
    md5sum "$CAIRN_SRC" "$got_cairn"
    exit 1
}
strings -a "$got_efi" | grep -F -q "kindling" || {
    echo "FAIL stick (BOOTX64.EFI has no kindling string)"
    exit 1
}
echo "ok   stick"
