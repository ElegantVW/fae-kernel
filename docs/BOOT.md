# Boot — the column

```
firmware (cerne-fw.asm) → loader (fused, same ROM) → kernel (start)
someone else's UEFI     → our BOOTX64.EFI          → start
```

Limine remains an optional hybrid ISO until you get bored of it.

## Our firmware (`make serial-fw`)

QEMU `-bios fw/cerne-fw.bin` (64KiB, reset at `0xFFFF0`). Serial:

```
cerne-fw
cerne-ld
fae-kernel
cerne: still only a spark
```

No Limine. No OVMF. The kernel is a flat binary at `0x200000` (`kernel.fw.bin`).

## Our EFI (`make serial-uefi`)

OVMF (other people's firmware, QEMU's copy of EDK2) loads **our** `BOOTX64.EFI`. Serial / ConOut:

```
cerne-efi
fae-kernel
cerne: still only a spark
```

## Limine ISO (`make serial`)

BIOS or UEFI firmware → Limine → ELF at higher-half. Crutch. Same `start()`.

## Host

`qemu-system-x86_64`, `nasm`, `edk2-ovmf`, `libisoburn` (for the Limine ISO).
