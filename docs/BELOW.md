# Below — the gate

Firmware → loader (off the disk) → Kindling spark. House light (G15–G24) is on.
Linux ELF waits on its own gate. This file staying green is still the law.

Run: `make below`  
Ten times: `make below-ten`

Limine (`make serial`) is a **crutch**, out of this gate.

| # | Gate | Proof | Status |
|---|---|---|---|
| G1 | Lilac first glyph (`ESC[95m`, VGA 13) | `make audit` lilac | yes |
| G2 | Honest well / dry / no-guest | 4M 8M 256M 1G 1025M 2G none wrong | yes |
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
| G17 | Reclaim — guard shut, drop replays same frames, count whole | reclaim in `make below` | yes |
| G18 | Tale — `tale` spark gleans `first-leaf` from the cairn at CPL3 | tale in `make below` | yes |
| G19 | Stow — `tale` re-inks the slate; second boot reads it kept | stow in `make below` | yes |
| G20 | Spawn — `wick` on a private cup + CR3 tells `second-leaf` | spawn in `make below` | yes |
| G21 | Grove — glyph + title on the glass after the well | grove in `make below` | yes |
| G22 | Iron Grove — ConOut glyph before GOP; serial wait bounded | efi greps `Grove` / `image` / `exit` | yes |
| G23 | Keyboard — PS/2 8042, IRQ1, house `read` fd 0 | ingle in `make below` | yes |
| G24 | Ingle — greeter spark waits for a line, says the fire is lit | ingle in `make below` / `make test` | yes |

Firmware extras (this sitting): VGA mode 3 by registers, 8×16 plane-2 font (`ok font` in `make below`), PIC ICW1–4, real-mode IVT + 32/64-bit IDT (`cerne-fw: trap`), 64-bit `lgdt`, `make below` includes `fw-trap`. Well maps through 3 GiB (`PDPT[0..2]`); `PDPT[3]` stays free for the LAPIC.

House calls (Gleam-only, not Linux): `docs/HOUSECALLS.md`. First set: `yield` / `exit` / `write(fd 1-2)` / `read(fd 0)` / `sleep` / `time` / `glean` / `stow` / `spawn` + shut `grant` / `flush` (`-EAGAIN`). `write` also paints the glass (VGA / GOP). Happy serial unchanged; `house-test` image prints `kindling: house ok`; `spawn-test` prints `kindling: spawn ok` then the second-leaf. After the well the glass shows the Grove glyph + title. EFI prints Grove on ConOut before touching GOP (IdeaPad has no COM1; Insyde hung inside GOP open).

Out of gate: storage beyond boot reads (writes, filesystems), SMP APs, LA57 well, trap recover, real iron besides QEMU.
