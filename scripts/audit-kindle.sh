#!/bin/sh
# Run the Kindling spark under several bowls. Exit 1 if a line is missing.
# The chain is real: firmware reads the loader off the disk (kindling.img),
# the loader reads the kernel. No -device loader anywhere.
set -eu
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="${HOME}/.cargo/bin:$PATH"
python3 scripts/mkfont.py
nasm -f bin -o fw/cerne-fw.bin fw/cerne-fw.asm
python3 scripts/romsum.py fw/cerne-fw.bin
nasm -f bin -o ld/cerne-ld.bin ld/cerne-ld.asm
make steel >/dev/null
FW=fw/cerne-fw.bin
IMG=kindling.img
WRONG=kindling-wrong.img
DISK="-drive if=ide,format=raw,file=$IMG"
# q35 (ICH9) has no legacy IDE behind 0x1F0 — attach a piix3-ide so our
# firmware has real ports to talk to. On pc the chipset already has them.
DISK_Q35="-device piix3-ide,id=p3 -drive if=none,id=kdisk,format=raw,file=$IMG -device ide-hd,drive=kdisk,bus=p3.0"
QEMU="${QEMU:-qemu-system-x86_64}"
fail=0

python3 scripts/mkimg.py --loader ld/cerne-ld.bin --kernel kernel/kernel.fw.bin --out "$IMG"
python3 scripts/mkimg.py --loader ld/cerne-ld.bin --kernel kernel/kernel.fw.bin --out "$WRONG" --wrong

run() {
  name=$1
  want=$2
  shift 2
  to=3
  if [ "$name" = 2G ]; then
    to=8
  fi
  got=$(timeout --foreground --signal=KILL "$to" "$@" 2>/dev/null | tr -d '\r' || true)
  printf '%s\n' "$got" | grep -F -q "$want" || {
    echo "FAIL $name (want: $want)"
    echo "$got" | tail -8
    fail=1
    return
  }
  echo "ok   $name"
}

run 256M "well 256 MiB" \
  "$QEMU" -M pc -m 256M -bios "$FW" $DISK \
  -display none -serial stdio -no-reboot -no-shutdown
run 8M "well 8 MiB" \
  "$QEMU" -M pc -m 8M -bios "$FW" $DISK \
  -display none -serial stdio -no-reboot -no-shutdown
run 4M "the well ran dry" \
  "$QEMU" -M pc -m 4M -bios "$FW" $DISK \
  -display none -serial stdio -no-reboot -no-shutdown
run 1G "well 1024 MiB" \
  "$QEMU" -M pc -m 1G -bios "$FW" $DISK \
  -display none -serial stdio -no-reboot -no-shutdown
run 1025M "well 1024 MiB" \
  "$QEMU" -M pc -m 1025M -bios "$FW" $DISK \
  -display none -serial stdio -no-reboot -no-shutdown
run 2G "well 2048 MiB" \
  "$QEMU" -M pc -m 2G -bios "$FW" $DISK \
  -display none -serial stdio -no-reboot -no-shutdown
run none "no guest at 0x200000" \
  "$QEMU" -M pc -m 256M -bios "$FW" \
  -display none -serial stdio -no-reboot -no-shutdown
run wrong "no guest at 0x200000" \
  "$QEMU" -M pc -m 256M -bios "$FW" -drive if=ide,format=raw,file="$WRONG" \
  -display none -serial stdio -no-reboot -no-shutdown
run lilac "$(printf '\033[95m')" \
  "$QEMU" -M pc -m 256M -bios "$FW" $DISK \
  -display none -serial stdio -no-reboot -no-shutdown
run q35 "well 256 MiB" \
  "$QEMU" -M q35 -m 256M -bios "$FW" $DISK_Q35 \
  -display none -serial stdio -no-reboot -no-shutdown

if [ "$fail" -ne 0 ]; then
  echo "kindling: audit failed"
  exit 1
fi
echo "kindling: audit ok"
