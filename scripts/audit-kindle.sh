#!/bin/sh
# Run the Kindling spark under several bowls. Exit 1 if a line is missing.
set -eu
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="${HOME}/.cargo/bin:$PATH"
make flint steel >/dev/null
FW=fw/cerne-fw.bin
K=kernel/kernel.fw.bin
QEMU="${QEMU:-qemu-system-x86_64}"
fail=0

run() {
  name=$1
  want=$2
  shift 2
  got=$(timeout --signal=KILL 3 "$@" 2>/dev/null | tr -d '\r' || true)
  printf '%s\n' "$got" | grep -F -q "$want" || {
    echo "FAIL $name (want: $want)"
    echo "$got" | tail -8
    fail=1
    return
  }
  echo "ok   $name"
}

run 256M "well 256 MiB" \
  "$QEMU" -M pc -m 256M -bios "$FW" -device loader,file="$K",addr=0x200000 \
  -display none -serial stdio -no-reboot -no-shutdown
run 8M "well 8 MiB" \
  "$QEMU" -M pc -m 8M -bios "$FW" -device loader,file="$K",addr=0x200000 \
  -display none -serial stdio -no-reboot -no-shutdown
run 4M "the well ran dry" \
  "$QEMU" -M pc -m 4M -bios "$FW" -device loader,file="$K",addr=0x200000 \
  -display none -serial stdio -no-reboot -no-shutdown
run 1G "well 1024 MiB" \
  "$QEMU" -M pc -m 1G -bios "$FW" -device loader,file="$K",addr=0x200000 \
  -display none -serial stdio -no-reboot -no-shutdown
run none "no guest at 0x200000" \
  "$QEMU" -M pc -m 256M -bios "$FW" \
  -display none -serial stdio -no-reboot -no-shutdown
run wrong "no guest at 0x200000" \
  "$QEMU" -M pc -m 256M -bios "$FW" -device loader,file="$K",addr=0x300000 \
  -display none -serial stdio -no-reboot -no-shutdown
run lilac "$(printf '\033[95m')" \
  "$QEMU" -M pc -m 256M -bios "$FW" -device loader,file="$K",addr=0x200000 \
  -display none -serial stdio -no-reboot -no-shutdown
run q35 "well 256 MiB" \
  "$QEMU" -M q35 -m 256M -bios "$FW" -device loader,file="$K",addr=0x200000 \
  -display none -serial stdio -no-reboot -no-shutdown

if [ "$fail" -ne 0 ]; then
  echo "kindling: audit failed"
  exit 1
fi
echo "kindling: audit ok"
