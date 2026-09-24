# Firmware contract — `cerne-fw`

64KiB ROM at `0xF0000`, reset at `0xFFFF0`. QEMU `-bios`. Kindling is a guest at `0x200000` (`KNDL` then `jmp kmain`).

## RAM the ROM owns

| Address | What |
|---|---|
| `0x0000` | IVT (we fill 0–31) |
| `0x1000` | PML4 |
| `0x2000` | PDPT |
| `0x3000` | PD |
| `0x4000` | PT for **0–2 MiB** (4 KiB pages) |
| `0x5000` | 32-bit IDT (until long mode) |
| `0x6000` | 64-bit IDT (until Kindling `lidt`) |
| `0x7000` | stack (grows down) |
| `0x8000` | FMAP |

Do not put Kindling’s cup in `0–4 MiB`.

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

Never a usable region covering `0xA0000–0xFFFFF`.

## Map

- **0–2 MiB:** 4 KiB pages. `0xA0000–0xFFFFF` present with PCD+PWT (hole / ROM / VGA).
- **2 MiB–ram_end:** 2 MiB pages.

## Guest

`dword[0x200000] == 'KNDL'` (`0x4C444E4B`). Jump `0x200004`. Else `kindling: no guest at 0x200000`.

## Traps

Real / 32 / 64 stubs print `cerne-fw: trap N` then `hlt`. Kindling replaces the 64-bit IDT after it stands.
