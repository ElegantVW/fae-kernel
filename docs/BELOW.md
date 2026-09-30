# Below — the gate

Firmware → loader (off the disk) → Kindling spark. **Phase 2 (ELF / syscalls) does not start until this file is boringly green.**

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
| G13 | Real chain — no `-device loader`, every bowl boots `kindling.img` | `make below` | yes |
| G14 | Bad KMAP refused and reported | kmap-bad in `make below` | yes |
| G15 | House gate — `int 0xE0` yield/write ok, unknown refuses | house in `make below` | yes |
| G16 | Ring-3 init — CPL3 `write`/`yield`/`exit` via house gate | ring3 in `make below` | yes |

Firmware extras (this sitting): VGA mode 3 by registers, PIC ICW1–4, real-mode IVT + 32/64-bit IDT (`cerne-fw: trap`), 64-bit `lgdt`, `make below` includes `fw-trap`.

House calls (Gleam-only, not Linux): `docs/HOUSECALLS.md`. First set v0: `yield` / `exit` / `write(fd 1-2)` / `sleep`-stub / `time` + shut `spawn` / `grant` / `flush` (`-EAGAIN`). Happy serial unchanged; `house-test` image prints `kindling: house ok`.

Out of gate: storage beyond boot reads (writes, filesystems), SMP APs, LA57 well, trap recover, real iron besides QEMU.
