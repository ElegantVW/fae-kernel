# Cairn — where Gleam keeps leaves and sparks

The cairn is the pile of stones on the disk: every leaf (file) and spark
(executable) the loader lays by the kernel. Packed at cast time by
`scripts/mkimg.py`, delivered to RAM by `ld/cerne-ld.asm`, read by Kindling.
Gleam-only. No FAT, no ext, no foreign shapes.

## Disk

```
LBA 0        KMAP  (magic, checksum, kernel_* — plus cairn_* below)
LBA 1..32    loader slot (16 KiB, 'LDOK' trailer)
LBA 33..     kernel (sector-padded)
...          cairn (sector-padded; location in KMAP)
```

## KMAP (disk LBA 0)

The loader reads only offsets 0–16 and checks the old checksum — those never
move, so the loader's trust is untouched. Cairn fields ride after, with their
own checksum the kernel checks:

| Off | Size | |
|---|---|---|
| 0 | u32 | `'KMAP'` (`0x50414D4B`) |
| 4 | u32 | checksum = `kernel_lba xor kernel_sectors xor kernel_xor xor 'KMAP'` |
| 8 | u32 | `kernel_lba` |
| 12 | u32 | `kernel_sectors` |
| 16 | u32 | `kernel_xor` |
| 20 | u32 | `cairn_lba` (0 = no cairn packed) |
| 24 | u32 | `cairn_sectors` |
| 28 | u32 | `cairn_sum` = `cairn_lba xor cairn_sectors xor cairn_xor xor 'CAIR'` |

`cairn_xor` is the XOR of every dword of the sector-padded cairn, same rite
as the kernel. A missing cairn (`cairn_lba == 0`) is honest, not an error —
`glean` then refuses with `-ENODEV`.

## Cairn image (at `cairn_lba`, in RAM at `0x100000`)

| Off | Size | |
|---|---|---|
| 0 | u32 | `'CAIR'` (`0x52494143`) |
| 4 | u32 | version (`1`) |
| 8 | u32 | `nleaves` |

Then `nleaves` records, each 4-aligned:

| Field | Size |
|---|---|
| kind | u32 (`0` = leaf/data, `1` = spark/executable) |
| namelen | u8 (1–64; house names are short) |
| name | bytes (no NUL, no `/` — flat grove, no paths yet) |
| datalen | u32 (≤ 1 MiB) |
| data | bytes |
| xor | u32 (XOR of every dword of the 4-padded data) |

The kernel verifies magic + walk + per-leaf xor on every read. A bad xor is
`-EILSEQ` territory reported as refusal, never partial bytes.

## Loader's oath

`cerne-ld` reads the KMAP, loads the kernel as before, then — only if
`cairn_sectors > 0` — loads the cairn straight to `0x100000` the same way
(128 sectors per command). A disk failure there refuses like any other:
cause line, house line, `hlt`. Never half a jump. The kernel re-verifies;
the loader only carries.

v0 rides the paved path only (BIOS → loader → kernel). EFI/Limine hands have
no cairn — `glean` there refuses `-ENODEV`, loudly documented, not silent.

## Words (house voice, no collisions)

- **cairn** — the store (stone pile for many pages; `fae-names.md` §6).
- **leaf** — a file (the Grove's leaves). Flat names, no paths yet.
- **spark** — an executable leaf (small fire that runs at CPL3).
- **glean** — house call 8: gather a leaf's bytes.
- **stow** — reserved: house call 9 (lay bytes down; G2b write path).
- **tale** — the first spark: speaks a leaf, then exits.
- **first-leaf** — the first leaf, packed at cast time.
