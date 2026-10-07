#!/bin/sh
# Full below gate + house gate (Gleam calls v0).
set -eu
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="${HOME}/.cargo/bin:$PATH"
FW=fw/cerne-fw.bin
DISK="-drive if=ide,format=raw,file=kindling.img"
QEMU="${QEMU:-qemu-system-x86_64}"

sh scripts/audit-kindle.sh

echo "---- font ----"
python3 scripts/check-font.py

echo "---- grove ----"
python3 scripts/check-grove.py

echo "---- trap6 ----"
make -C kernel trap6-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.trap.bin --out kindling-trap.img
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" -drive if=ide,format=raw,file=kindling-trap.img \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "kindling: trap 6" || {
  echo "FAIL trap6"
  echo "$got" | tail -12
  exit 1
}
echo "ok   trap6"
# restore a non-trap kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- house ----"
make -C kernel house-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.house.bin --out kindling-house.img
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" -drive if=ide,format=raw,file=kindling-house.img \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "kindling: house ok" || {
  echo "FAIL house"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "well 256 MiB" || {
  echo "FAIL house (no well line)"
  echo "$got" | tail -12
  exit 1
}
echo "ok   house"
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- ring3 ----"
make -C kernel ring3-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.ring3.bin --out kindling-ring3.img
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" -drive if=ide,format=raw,file=kindling-ring3.img \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "kindling: init ok" || {
  echo "FAIL ring3"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "kindling: init slept" || {
  echo "FAIL ring3 (no init slept — timer IRQ from CPL3)"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "kindling: gleam exit 0" || {
  echo "FAIL ring3 (no gleam exit 0)"
  echo "$got" | tail -12
  exit 1
}
echo "ok   ring3"
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- reclaim ----"
make -C kernel reclaim-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.reclaim.bin --out kindling-reclaim.img
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" -drive if=ide,format=raw,file=kindling-reclaim.img \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "kindling: reclaim ok" || {
  echo "FAIL reclaim"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "well 256 MiB" || {
  echo "FAIL reclaim (no well line)"
  echo "$got" | tail -12
  exit 1
}
echo "ok   reclaim"
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- tale ----"
nasm -f bin -o spark/tale.bin spark/tale.asm
make -C kernel tale-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.tale.bin --out kindling-tale.img \
  --spark tale=spark/tale.bin --leaf first-leaf=spark/first-leaf.txt \
  --leaf slate=spark/slate.txt
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" -drive if=ide,format=raw,file=kindling-tale.img \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "kindling remembers the reset" || {
  echo "FAIL tale"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "stowed" || {
  echo "FAIL tale (no stowed on first boot)"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "kindling: gleam exit 0" || {
  echo "FAIL tale (no gleam exit 0)"
  echo "$got" | tail -12
  exit 1
}
echo "ok   tale"
echo "---- stow ----"
# Same disk, second boot: the slate must read inked without re-stowing.
# That is the proof the ink survived on iron, not in RAM.
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" -drive if=ide,format=raw,file=kindling-tale.img \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "ink holds" || {
  echo "FAIL stow (ink did not survive)"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "kept" || {
  echo "FAIL stow (no kept on second boot)"
  echo "$got" | tail -12
  exit 1
}
if printf '%s\n' "$got" | grep -F -q "stowed"; then
  echo "FAIL stow (re-stowed on second boot — ink never landed)"
  echo "$got" | tail -12
  exit 1
fi
echo "ok   stow"
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- spawn ----"
nasm -f bin -o spark/wick.bin spark/wick.asm
nasm -f bin -o spark/ember.bin spark/ember.asm
make -C kernel spawn-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.spawn.bin --out kindling-spawn.img \
  --spark ember=spark/ember.bin --spark wick=spark/wick.bin \
  --leaf second-leaf=spark/second-leaf.txt
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" -drive if=ide,format=raw,file=kindling-spawn.img \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "kindling: spawn ok" || {
  echo "FAIL spawn"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "the cup is its own" || {
  echo "FAIL spawn (no second-leaf)"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "stayed" || {
  echo "FAIL spawn (no stayed)"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "kindling: gleam exit 0" || {
  echo "FAIL spawn (no gleam exit 0)"
  echo "$got" | tail -12
  exit 1
}
echo "ok   spawn"
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- splanc ----"
nasm -f bin -o spark/ember.bin spark/ember.asm
nasm -f bin -o spark/splanc.bin spark/splanc.asm
make -C kernel spawn-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.spawn.bin --out kindling-splanc.img \
  --spark ember=spark/ember.bin --spark splanc=spark/splanc.bin
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" -drive if=ide,format=raw,file=kindling-splanc.img \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "kindling: spawn ok" || {
  echo "FAIL splanc (no spawn ok)"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "kindling: trap 6" || {
  echo "FAIL splanc (no trap 6)"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "the spark went out" || {
  echo "FAIL splanc (no went out)"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "stayed" && {
  echo "FAIL splanc (stayed — a lie)"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "kindling: gleam exit 1" || {
  echo "FAIL splanc (no gleam exit 1)"
  echo "$got" | tail -12
  exit 1
}
echo "ok   splanc"
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- ingle ----"
nasm -f bin -o spark/ingle.bin spark/ingle.asm
make -C kernel ingle-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.ingle.bin --out kindling-ingle.img \
  --spark ingle=spark/ingle.bin
python3 scripts/check-ingle.py
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- ingle-leaf ----"
nasm -f bin -o spark/ingle.bin spark/ingle.asm
nasm -f bin -o spark/leaf.bin spark/leaf.asm
make -C kernel ingle-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.ingle.bin --out kindling-ingle-leaf.img \
  --spark ingle=spark/ingle.bin --spark leaf=spark/leaf.bin \
  --leaf LEAF=spark/LEAF.txt
python3 scripts/check-ingle-leaf.py
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- ingle-keeper ----"
nasm -f bin -o spark/ingle.bin spark/ingle.asm
nasm -f bin -o spark/keeper.bin spark/keeper.asm
python3 -c "from pathlib import Path; Path('spark/hand.bin').write_bytes(bytes(32)); Path('spark/hands.bin').write_bytes(bytes(716)); Path('spark/twin.bin').write_bytes(bytes(716))"
make -C kernel ingle-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.ingle.bin --out kindling-ingle-keeper.img \
  --spark ingle=spark/ingle.bin --spark keeper=spark/keeper.bin \
  --leaf hand=spark/hand.bin --leaf hands=spark/hands.bin \
  --leaf twin=spark/twin.bin
python3 scripts/check-ingle-keeper.py
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- leaf ----"
nasm -f bin -o spark/leaf.bin spark/leaf.asm
make -C kernel leaf-bin
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.leaf.bin --out kindling-leaf.img \
  --spark leaf=spark/leaf.bin --leaf LEAF=spark/LEAF.txt
python3 scripts/check-leaf.py
# restore the paved kernel for later steps
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img

echo "---- fmap-bad ----"
python3 scripts/mkfont.py
nasm -f bin -DAUDIT_BAD_FMAP -o "$FW" fw/cerne-fw.asm
python3 scripts/romsum.py "$FW"
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" $DISK \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "the well ran dry" || {
  echo "FAIL fmap-bad"
  echo "$got" | tail -12
  exit 1
}
echo "ok   fmap-bad"
nasm -f bin -o "$FW" fw/cerne-fw.asm
python3 scripts/romsum.py "$FW"

echo "---- fw-trap ----"
nasm -f bin -DAUDIT_FW_TRAP -o "$FW" fw/cerne-fw.asm
python3 scripts/romsum.py "$FW"
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" $DISK \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "cerne-fw: trap" || {
  echo "FAIL fw-trap"
  echo "$got" | tail -12
  exit 1
}
echo "ok   fw-trap"
nasm -f bin -o "$FW" fw/cerne-fw.asm
python3 scripts/romsum.py "$FW"

echo "---- kmap-bad ----"
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling-bad.img --bad
got=$(timeout --foreground --signal=KILL 3 "$QEMU" -M pc -m 256M \
  -bios "$FW" -drive if=ide,format=raw,file=kindling-bad.img \
  -display none -serial stdio -no-reboot -no-shutdown 2>/dev/null | tr -d '\r' || true)
printf '%s\n' "$got" | grep -F -q "bad kmap" || {
  echo "FAIL kmap-bad"
  echo "$got" | tail -12
  exit 1
}
printf '%s\n' "$got" | grep -F -q "no guest at 0x200000" || {
  echo "FAIL kmap-bad (no refusal line)"
  echo "$got" | tail -12
  exit 1
}
echo "ok   kmap-bad"

echo "---- efi ----"
make efi >/dev/null
rm -rf esp
mkdir -p esp/EFI/BOOT
cp -f kernel/BOOTX64.EFI esp/EFI/BOOT/
VARS=$(ls /usr/share/edk2/x64/OVMF_VARS.4m.fd /usr/share/edk2/x64/OVMF_VARS.fd 2>/dev/null | head -1)
CODE=$(ls /usr/share/edk2/x64/OVMF_CODE.4m.fd /usr/share/edk2/x64/OVMF_CODE.fd 2>/dev/null | head -1)
cp -f "$VARS" ovmf_vars.fd
got=$(timeout --foreground --signal=KILL 12 "$QEMU" -M q35 -m 256M -display none \
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
printf '%s\n' "$got" | grep -F -q "Grove" || {
  echo "FAIL efi grove"
  echo "$got" | tail -16
  exit 1
}
printf '%s\n' "$got" | grep -Fx -q "image" || {
  echo "FAIL efi image"
  echo "$got" | tail -16
  exit 1
}
printf '%s\n' "$got" | grep -Fx -q "exit" || {
  echo "FAIL efi exit"
  echo "$got" | tail -16
  exit 1
}
printf '%s\n' "$got" | grep -F -q "kindling: no ingle" || {
  echo "FAIL efi no-ingle"
  echo "$got" | tail -16
  exit 1
}
echo "ok   efi"

echo "---- efi-ingle ----"
nasm -f bin -o spark/ingle.bin spark/ingle.asm
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img \
  --spark ingle=spark/ingle.bin --cairn-out spark/cairn.bin
rm -rf esp
mkdir -p esp/EFI/BOOT
cp -f kernel/BOOTX64.EFI esp/EFI/BOOT/
cp -f spark/cairn.bin esp/EFI/BOOT/CAIRN
python3 scripts/check-efi-ingle.py

echo "---- efi-usb ----"
python3 scripts/check-efi-usb.py

echo "---- efi-usb-hub ----"
python3 scripts/check-efi-usb-hub.py

echo "---- efi-msc ----"
python3 scripts/check-efi-msc.py

echo "---- efi-leaf ----"
make -C kernel efi-leaf >/dev/null
python3 scripts/check-efi-leaf.py

echo "---- efi-stow ----"
make -C kernel efi-tale >/dev/null
python3 scripts/check-efi-stow.py

echo "---- efi-keeper ----"
python3 scripts/check-efi-keeper.py

echo "---- efi-sleep ----"
python3 scripts/check-efi-sleep.py

echo "---- efi-glass ----"
python3 scripts/check-efi-glass.py

echo "---- paved cairn ----"
nasm -f bin -o spark/ingle.bin spark/ingle.asm
nasm -f bin -o spark/leaf.bin spark/leaf.asm
nasm -f bin -o spark/keeper.bin spark/keeper.asm
python3 -c "from pathlib import Path; Path('spark/hand.bin').write_bytes(bytes(32)); Path('spark/hands.bin').write_bytes(bytes(716)); Path('spark/twin.bin').write_bytes(bytes(716))"
make steel >/dev/null
python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
  --kernel kernel/kernel.fw.bin --out kindling.img \
  --spark ingle=spark/ingle.bin --spark leaf=spark/leaf.bin \
  --spark keeper=spark/keeper.bin --leaf LEAF=spark/LEAF.txt \
  --leaf hand=spark/hand.bin --leaf hands=spark/hands.bin \
  --leaf twin=spark/twin.bin --cairn-out spark/cairn.bin

echo "---- stick ----"
sh scripts/check-stick.sh

echo "kindling: below ok"
