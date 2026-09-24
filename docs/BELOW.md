# Below — the gate

Firmware → fused loader → Kindling spark. **Phase 2 (ELF / syscalls) does not start until this file is boringly green.**

Run: `make below`  
Ten times: `make below-ten`

Limine (`make serial`) is a **crutch**, out of this gate.

| # | Gate | Proof | Status |
|---|---|---|---|
| G1 | Lilac first glyph (`ESC[95m`, VGA 13) | `make audit` lilac | yes |
| G2 | Honest well / dry / no-guest | 4M 8M 256M 1G none wrong | yes |
| G3 | GDT + IDT; `trap 6` on `ud2` | trap6 in `make below` | yes |
| G4 | Cup, canary, RSP then CR3 | well line | yes |
| G5 | `pc` + `q35`; EFI well | q35 + efi in `make below` | yes |
| G6 | FPU/SSE on | `fninit` + `movaps` then well | yes |
| G7 | PIC masked | firmware + kernel | yes |
| G8 | Firmware maps measured well only | 8M → 8 MiB | yes |
| G9 | FMAP magic + checksum | fmap-bad in `make below` | yes |
| G10 | Limine out of gate | crutch | **crutch** |
| G11 | `make below` × 10 | `make below-ten` | **yes — ten ok** |
| G12 | This file + Flint handoff | `docs/BELOW.md` | yes |

Firmware extras (this sitting): VGA mode 3 by registers, PIC ICW1–4, real-mode IVT + 32/64-bit IDT (`cerne-fw: trap`), 64-bit `lgdt`, `make below` includes `fw-trap`.

Out of gate: disk, SMP APs, LA57 well, trap recover, real iron besides QEMU.
