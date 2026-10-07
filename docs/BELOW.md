# Below — the gate

Firmware → loader (off the disk) → Kindling spark. House light (G15–G38) is on.
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
| G25 | Paved ingle — BIOS `make image` packs the spark; EFI plants `EFI/BOOT/CAIRN` | efi-ingle in `make below` | yes |
| G25b | Iron cairn — walk USB child FS + LoadedImageDevicePath; glass `no ingle` on miss | efi greps `no ingle`; efi-ingle still greets | yes |
| G25c | Iron ingle wait — poll 8042; `hlt` only with PIT; glass cursor + `kindling` | efi-ingle sendkey still greets | yes |
| G26 | Post-well glass — map PE+GOP before CR3; `kindling` on show(); efi-glass + stick | efi-glass `xp`; stick `cmp` | yes |
| G27 | USB HID boot keyboard — xHCI poll into house `read`; 8042 stays | efi-usb `usb kbd` then sendkey; efi-ingle still greets | yes |
| G27b | Iron xHCI — PPC, protocol slot type, one hub hop; glass `xhci` / `usb kbd` / `no usb` | efi-usb root kbd; efi-usb-hub one hop; efi-ingle still greets | yes |
| G28 | One clock — HPET or polled PIT, TSC ms; `sleep`/`time` on EFI | efi-sleep `tick` + `efi slept`; ring3 `init slept` | yes |
| G29 | USB MSC BOT — scan every port, READ CAPACITY, glass `msc` | efi-msc kbd port 1 + storage port 2; `msc` then sendkey | yes |
| G29b | Iron MSC — clock settle + rescan after HCRST; miss names the step | efi-msc still `msc`; glass `no ccs`/`no dev`/`no bot`/`no cap` | yes |
| G29c | Iron MSC wait — clock timeouts, one reset try, BAR sanity, EP0 8 | efi-msc/efi-usb still greet; well→xhci is a blink | yes |
| G29d | Iron MSC PORTSC — Linux-neutral writes; miss is rst/addr/desc | efi-msc still `msc`; glass `no rst`/`no addr`/`no desc` | yes |
| G29e | Iron PORTSC PED — bit 1 is RW1CS; never write 1 after reset | efi-msc still `msc`; PR self-clear on USB2 | yes |
| G29f | Iron EP0 — xHCI 1.0 control TDs (TRT, no Chain, hold first TRB) | efi-msc still `msc`; glass `no desc` was GET_DESCRIPTOR | yes |
| G29g | Iron MSC READ(10) — LBA 0 boot sector; glass `fat` / `no fat` | efi-msc `msc` then `fat`; sendkey still greets | yes |
| G29h | Iron FAT partition — MBR/GPT walk to the FAT boot sector | efi-msc MBR + FAT32 at LBA 2048; glass `fat` | yes |
| G30 | The light remains — one Light; ember kindles wick and comes home | spawn greps `stayed` then `gleam exit 0` | yes |
| G31 | The spark went out — guest `#UD` smoors; Light's spawn refuses | splanc in `make below`: `trap 6`, `the spark went out`, `gleam exit 1` | yes |
| G32 | Glean a leaf from the volume — cairn first, then FAT root by Gleam name | efi-msc FAT32 + `LEAF`; serial `the volume speaks` | yes |
| G33 | Spark `leaf` — gleans `LEAF`, writes the page, reads until `q` | leaf in `make below`; efi-leaf HID `q` then `gleam exit 0` | yes |
| G34 | Ingle keeps the Light — kindles `leaf`, `the light remains`, glass last word | ingle-leaf in `make below`; ingle tests send **q** | yes |
| G35 | Spawn from the volume — cairn first, then FAT root, 64 KiB raw bin | efi-msc ingle-only cairn + LFN `leaf`; `the light remains` | yes |
| G36 | Stow on the volume — WRITE(10)+reread, KINDLING `85C7-AA81`, exact measure | efi-stow two-boot: `stowed` then `kept` | yes |
| G37 | Keeper — one name on the volume; next boot ingle greets it | ingle-keeper + efi-keeper two-boot: `gil`, never re-asks | yes |
| G38 | The word is visible — keeper echoes name keys; glass backspace | ingle-keeper / efi-keeper: `gix` BS `l` lands `gil` | yes |

Firmware extras (this sitting): VGA mode 3 by registers, 8×16 plane-2 font (`ok font` in `make below`), PIC ICW1–4, real-mode IVT + 32/64-bit IDT (`cerne-fw: trap`), 64-bit `lgdt`, `make below` includes `fw-trap`. Well maps through 3 GiB (`PDPT[0..2]`); `PDPT[3]` stays free for the LAPIC.

House calls (Gleam-only, not Linux): `docs/HOUSECALLS.md`. First set: `yield` / `exit` / `write(fd 1-2)` / `read(fd 0)` / `sleep` / `time` / `glean` / `stow` / `spawn` + shut `grant` / `flush` (`-EAGAIN`). `glean` takes the cairn first, then a live FAT volume's root by Gleam name. `spawn` gathers a named spark the same way (64 KiB, raw `nasm -f bin`). `stow` re-inks a leaf the same way (exact measure; WRITE(10)+READ(10) compare on FAT; KINDLING `85C7-AA81` only). Spark `leaf` tells that page and waits for `q`. Paved `ingle` greets an inked `hand`, kindles `leaf` after the fire is lit, and writes `the light remains` when it smoors. An empty hand kindles `keeper` (Setup Assistant). No-Light `exit` paints serial and the glass `kindling: gleam exit N`. `write` also paints the glass (VGA / GOP). Happy serial unchanged; `house-test` image prints `kindling: house ok`; `spawn-test` prints `kindling: spawn ok`, the second-leaf, then `stayed`. Guest `#UD`/`#PF`/`#GP` print `kindling: trap N` and `the spark went out`; with a Light, `spawn` returns `-EIO`. After the well the glass shows the Grove glyph + title. EFI prints Grove on ConOut before touching GOP (IdeaPad has no COM1; Insyde hung inside GOP open).

Out of gate: KINDLOG, creating or growing FAT files, SMP APs, LA57 well, resume at a fault RIP, UHCI/EHCI/mice, deep hubs (xHCI HID boot keyboard, one hub hop, BOT MSC READ CAPACITY, READ(10) of a FAT boot sector, WRITE(10) of a KINDLING sector, `glean` of a named leaf from the volume root, `spawn` of a named spark from that root, and `stow` of a named leaf onto that root are in-gate; 8042 poll stays). Guest traps smoor into the Light (G31); ring-0 traps stay fatal.
