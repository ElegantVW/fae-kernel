#!/bin/sh
# Full below gate. No phase 2.
set -eu
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="${HOME}/.cargo/bin:$PATH"

sh scripts/audit-kindle.sh

echo "---- trap6 ----"
make -C kernel trap6-bin
make flint
got=$(timeout --foreground --signal=KILL 3 qemu-system-x86_64 -M pc -m 256M \
  -bios fw/cerne-fw.bin -device loader,file=kernel/kernel.trap.bin,addr=0x200000 \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "kindling: trap 6" || {
  echo "FAIL trap6"
  echo "$got" | tail -12
  exit 1
}
echo "ok   trap6"
# restore a non-trap kernel for later steps
make steel >/dev/null

echo "---- fmap-bad ----"
nasm -f bin -DAUDIT_BAD_FMAP -o fw/cerne-fw.bin fw/cerne-fw.asm
python3 scripts/romsum.py fw/cerne-fw.bin
got=$(timeout --foreground --signal=KILL 3 qemu-system-x86_64 -M pc -m 256M \
  -bios fw/cerne-fw.bin -device loader,file=kernel/kernel.fw.bin,addr=0x200000 \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "the well ran dry" || {
  echo "FAIL fmap-bad"
  echo "$got" | tail -12
  exit 1
}
echo "ok   fmap-bad"
nasm -f bin -o fw/cerne-fw.bin fw/cerne-fw.asm
python3 scripts/romsum.py fw/cerne-fw.bin

echo "---- fw-trap ----"
nasm -f bin -DAUDIT_FW_TRAP -o fw/cerne-fw.bin fw/cerne-fw.asm
python3 scripts/romsum.py fw/cerne-fw.bin
got=$(timeout --foreground --signal=KILL 3 qemu-system-x86_64 -M pc -m 256M \
  -bios fw/cerne-fw.bin -device loader,file=kernel/kernel.fw.bin,addr=0x200000 \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "cerne-fw: trap" || {
  echo "FAIL fw-trap"
  echo "$got" | tail -12
  exit 1
}
echo "ok   fw-trap"
nasm -f bin -o fw/cerne-fw.bin fw/cerne-fw.asm
python3 scripts/romsum.py fw/cerne-fw.bin

echo "---- efi ----"
make efi >/dev/null
rm -rf esp
mkdir -p esp/EFI/BOOT
cp -f kernel/BOOTX64.EFI esp/EFI/BOOT/
VARS=$(ls /usr/share/edk2/x64/OVMF_VARS.4m.fd /usr/share/edk2/x64/OVMF_VARS.fd 2>/dev/null | head -1)
CODE=$(ls /usr/share/edk2/x64/OVMF_CODE.4m.fd /usr/share/edk2/x64/OVMF_CODE.fd 2>/dev/null | head -1)
cp -f "$VARS" ovmf_vars.fd
got=$(timeout --foreground --signal=KILL 12 qemu-system-x86_64 -M q35 -m 256M -display none \
  -serial stdio -no-reboot \
  -drive if=pflash,format=raw,unit=0,readonly=on,file="$CODE" \
  -drive if=pflash,format=raw,unit=1,file=ovmf_vars.fd \
  -drive format=raw,file=fat:rw:esp 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "efi map" || {
  echo "FAIL efi map"
  echo "$got" | tail -16
  exit 1
}
printf '%s\n' "$got" | grep -F -q "well" || {
  echo "FAIL efi well"
  echo "$got" | tail -16
  exit 1
}
echo "ok   efi"

echo "kindling: below ok"
