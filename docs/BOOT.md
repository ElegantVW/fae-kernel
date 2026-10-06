# Boot — the column

```
firmware (cerne-fw.asm) → loader (cerne-ld, LBA 1..32 of the disk) → kernel (start)
someone else's UEFI     → our BOOTX64.EFI                          → start
```

Limine remains an optional hybrid ISO until you get bored of it.

## Our firmware (`make serial-fw`)

QEMU `-bios fw/cerne-fw.bin` (64KiB, reset at `0xFFFF0`) with the boot disk
(`kindling.img`, cast by `make image`). Serial:

```
cerne-fw
cerne-ld
fae-kernel
kindling: still only a spark
```

No Limine. No OVMF. No `-device loader` — nothing is cheated into RAM. The
chain earns every byte:

1. **firmware** reads LBA 1..32 of the disk to `0x9000` and checks the slot
   trailer `'LDOK'` that `scripts/mkimg.py` writes at the slot's end.
2. **loader** (`ld/cerne-ld.asm`, self-contained: own ATA PIO, own serial)
   reads the KMAP (LBA 0), loads the kernel straight to `0x200000`, checks
   its XOR and `'KNDL'`, jumps `0x200004`.

Disk: `LBA 0` KMAP · `LBA 1..32` loader (16KiB slot) · `LBA 33..` kernel ·
cairn (ingle, when packed). `make image` lays `--spark ingle` and writes
`spark/cairn.bin`. After the well, the happy kernel lights `ingle` when the
cairn has it; missing cairn is a quiet halt.
Contracts in `docs/FIRMWARE.md`. RAM size is CMOS (this ROM *is* the BIOS —
there is no `int 0x15`). First glyph is Lilac (VGA 13 + serial `ESC[95m`).
`make hearth-see` opens a window.

The hand path is blessed: `nasm -f bin fw/cerne-fw.asm -o fw/cerne-fw.bin`,
`nasm -f bin ld/cerne-ld.asm -o ld/cerne-ld.bin`, `make steel`, then
`python3 scripts/mkimg.py` and QEMU `-bios … -drive if=ide,…` as in
`make kindle`. Same fire. `strings` on the ROM hides two murmurs (`kindling
remembers the reset`, `the jump is the vow`) — they do not print on serial.

On q35 there is no legacy IDE behind `0x1F0`; attach one (`-device
piix3-ide …`, see `scripts/audit-kindle.sh`) or the firmware honestly reports
`cerne-fw: no disk`. On pc the chipset already has the ports.

## Our EFI (`make serial-uefi`)

OVMF (other people's firmware, QEMU's copy of EDK2) loads **our** `BOOTX64.EFI`. Serial / ConOut:

```
cerne-efi
\  |  /
 \| |/ 
---+---
 / | \ 
/  |  \
Grove
image
cairn
gop
exit
fae-kernel
kindling: still only a spark
```

`make serial-uefi` copies `EFI/BOOT/CAIRN` next to `BOOTX64.EFI`. A missing
cairn skips the `cairn` line; Grove still paints, then the glass says
`kindling` (from `show`) and `no ingle`. After the well, `ingle` runs when
the cairn planted. The well maps the PE and GOP before `mov cr3` so a
high Insyde load does not freeze the pre-EBS picture. After the well,
xHCI is taken (USBLEGSUP, halt, reset, run); glass names the step:
`xhci`, then `usb kbd` when a HID boot keyboard is queued (root port
or one hub hop). Miss is `no xhci` (no PCI class `0x0C0330`) or
`xhci` then `no usb` (HC ran, no keyboard). After `tick`, `msc` when a
BOT mass-storage device answers READ CAPACITY (every root port and hub
child is scanned; HID does not stop the walk). Miss is `no msc`.

ConOut (the laptop panel) carries `cerne-efi`, the Grove sigil, `image` /
`cairn` / `gop` / `exit`. COM1 carries `fae-kernel` and the well line. OVMF
mirrors ConOut onto QEMU serial; IdeaPad does not.

## Limine ISO (`make serial`)

BIOS or UEFI firmware → Limine → ELF at higher-half. Crutch. Same `start()`.

## Host

`qemu-system-x86_64`, `nasm`, `edk2-ovmf`, `libisoburn` (for the Limine ISO).
