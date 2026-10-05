# Firmware contract — `cerne-fw`

64KiB ROM at `0xF0000`, reset at `0xFFFF0`. QEMU `-bios`. Kindling is a guest at `0x200000` (`KNDL` then `jmp kmain`).

## RAM the ROM owns

| Address | What |
|---|---|
| `0x0000` | IVT (we fill 0–31) |
| `0x1000` | PML4 |
| `0x2000` | PDPT |
| `0x3000` | PD for **0–1 GiB** (`PDPT[0]`; `PD[0]` is the 4K PT) |
| `0x4000` | PT for **0–2 MiB** (4 KiB pages) |
| `0x5000` | 32-bit IDT (until long mode) |
| `0x6000` | 64-bit IDT (until Kindling `lidt`) |
| `0x7000` | stack (grows down) |
| `0x8000` | FMAP |
| `0x8020` | font readback (`1` = plane 2 took `'*'`) |
| `0x8400` | KMAP scratch (the loader reads disk LBA 0 here) |
| `0x9000` | `cerne-ld`, the loader — 16KiB slot from disk LBA 1..32 |
| `0xD000` | PD for **1–2 GiB** (`PDPT[1]`, when CMOS needs it) |
| `0xE000` | PD for **2–3 GiB** (`PDPT[2]`, when CMOS needs it) |

`PDPT[3]` stays empty so Kindling can plant the LAPIC window. RAM past 3 GiB
is not mapped this sitting; `probe_cap` stops at the first not-present entry
instead of taking a `#PF`.

Do not put Kindling's cup in `0–4 MiB`. `0x8400` and `0x9000–0xCFFF` belong to
the loader and are free RAM again once the guest takes the jump.

## GDT (ROM)

| Selector | Meaning |
|---|---|
| `0x08` | 32-bit code |
| `0x10` | 32-bit data |
| `0x18` | 64-bit code |
| `0x20` | 64-bit data |

Kindling reloads its own GDT (CS `0x08`) on the BIOS path.

## FMAP (`0x8000`)

| Off | Size | |
|---|---|---|
| 0 | u32 | `'FMAP'` |
| 4 | u32 | checksum = `ram_end xor nreg xor 'KNDL'` |
| 8 | u32 | `ram_end` (first byte past usable RAM) |
| 12 | u32 | `nreg` (2) |
| 16 | u32+u32 | region 0: `0`, `0x9F000` |
| 24 | u32+u32 | region 1: `0x100000`, `ram_end - 0x100000` |
| 32 | u8 | font ok (`1` after plane-2 readback of `'*'`) |

Never a usable region covering `0xA0000–0xFFFFF`. Byte 32 is not in the
checksum — it is a side flag, not a region.

## Map

- **0–2 MiB:** 4 KiB pages. `0xA0000–0xFFFFF` present with PCD+PWT (hole / ROM / VGA).
- **2 MiB–ram_end:** 2 MiB pages, across `PDPT[0..2]` (cap 3 GiB). `PDPT[3]` free.
- VGA plane 2: 8×16 ASCII (`fw/font8x16.bin`, flint via `scripts/mkfont.py`).
  Readback miss prints `cerne-fw: no font` (happy serial stays quiet).
  Attribute controller 0–15 is identity so DAC index 13 is Lilac.

## Loader (`ld/cerne-ld.asm`)

The firmware reads **LBA 1..32** of the boot disk to `0x9000` and checks the
slot trailer `dword[0xCFFC] == 'LDOK'` (`0x4B4F444C`) — a truncated or absent
slot is `cerne-fw: no loader`, a floating bus `cerne-fw: no disk`; either way
`kindling: no guest at 0x200000` and `hlt`. Never half a jump.

On success it jumps `0x9000` in long mode: segments `0x20`, `RSP 0x7000`, this
ROM’s GDT / IDT / page tables still standing, serial ready. The loader is
self-contained (own ATA PIO, own serial) and owes the guest: read the KMAP,
load the kernel straight to `0x200000` (128 sectors per command), check its
XOR and `'KNDL'`, jump `0x200004`. Refusals print `cerne-ld: no disk` /
`cerne-ld: bad kmap` / `cerne-ld: kernel checksum`, then the house line.

## KMAP (disk LBA 0)

| Off | Size | |
|---|---|---|
| 0 | u32 | `'KMAP'` (`0x50414D4B`) |
| 4 | u32 | checksum = `kernel_lba xor kernel_sectors xor kernel_xor xor 'KMAP'` |
| 8 | u32 | `kernel_lba` |
| 12 | u32 | `kernel_sectors` (the loader reads `kernel_sectors * 512` bytes) |
| 16 | u32 | `kernel_xor` — XOR of every dword of the sector-padded kernel |

Written by `scripts/mkimg.py`. `--bad` corrupts the checksum (gate G14),
`--wrong` points it at empty disk, consistently (gate G2 “wrong” — the `'KNDL'`
check is the one that catches that).

## Guest

`dword[0x200000] == 'KNDL'` (`0x4C444E4B`). Jump `0x200004`. Else `kindling: no guest at 0x200000`. Unchanged — the EFI and Limine paths hand over the same way.

## Traps

Real / 32 / 64 stubs print `cerne-fw: trap N` then `hlt`. Kindling replaces the 64-bit IDT after it stands.
