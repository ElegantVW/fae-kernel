# fae-kernel changelog

## ingle-wait-g25c (2026-10-05)

- Iron showed a quiet cursor under Grove: cairn planted and ingle wrote, then `read` `hlt`'d. EFI never programs the PIT, and laptop IRQ1 often dies after ExitBootServices, so that `hlt` never wakes. `read` now polls the 8042 data port and `pause`s unless the PIT is armed. GOP log sits at Grove's margin with a parchment bar cursor. After the well the glass says `kindling`, and `no kbd` if the 8042 probe refused.

## efi-cairn-g25b (2026-10-05)

- Iron (IdeaPad Insyde) showed Grove twice and nothing else: GOP covers ConOut breadcrumbs, and the cairn never planted. LoadedImage.device() often has no SimpleFileSystem (USB child does). Walk: GetProtocol on the device, LocateDevicePath on that path, LocateDevicePath on LoadedImageDevicePath (the HD node lives on the file path), then every SimpleFileSystem handle until `CAIRN` opens. Nested `EFI/BOOT/CAIRN` open. AllocateAddress 1MB may refuse 512 KiB; shrink to 64/16/4 KiB, else AnyPages + copy after EBS when the slot is free and not the running image. Miss writes `no ingle` on the glass (no COM1). `make below` EFI (no CAIRN) greps that line.

## ingle-g25 (2026-10-05)

- Paved boots carry `ingle`. `make image` packs `--spark ingle` and writes `spark/cairn.bin` (`--cairn-out`). After the well, the happy kernel calls `light_ingle`: missing cairn or spark is a quiet halt, never a FAIL line.
- EFI reads `\EFI\BOOT\CAIRN` with GetProtocol SimpleFileSystem (not exclusive), AllocateAddress `0x100000`, plants KMAP scratch at `0x8400` after ExitBootServices. Missing file is honest — Grove still paints. ConOut prints `cairn` when the blob landed. Kindling's GDT (user segments + TSS) and 8259 remap (0x20/0x28) now run on the EFI path so CPL3 `ingle` can iretq and IRQ1 lands on the house vector. Asm-called `house_entry` / ticks are `extern "sysv64"` — the UEFI target's `extern "C"` is win64, which was turning `write` into `grant` and `read` into `exit`.
- `make below` += `efi-ingle` (OVMF + sendkey). KINDLING stick takes `BOOTX64.EFI` + `EFI/BOOT/CAIRN`.

## ingle-g24 (2026-10-05)

- House `read` (call 10) on fd 0: PS/2 8042, IRQ1, scancode set 1 → ASCII. No controller is `-EAGAIN`. Wrong fd is `-EPERM`. Blocks until a byte when live.
- Spark `ingle` writes its name, reads a line, writes `the fire is lit`, `exit 0`. `make below` += `ingle` (G23/G24). `make test` runs it via monitor `sendkey ret`.

## iron-grove-g22 (2026-10-05)

- EFI ConOut prints the Grove sigil + title *before* any GOP open. IdeaPad 3 15ITL6 (Insyde) froze after `cerne-efi` inside GOP; the panel is the only transcript. Breadcrumbs `image` / `gop` / `exit`. GOP paint still happens before `ExitBootServices` when the open returns.
- `serial_put` gives up if COM1 LSR never ready (no UART on the laptop). GOP show is glyphs only — no full-framebuffer UC fill.
- `make below` EFI greps `Grove` / `image` / `exit`. `make test` runs the throwaway VMs (happy..spawn) plus grove. Stick update is the iron verdict.

## grove-g21 (2026-10-05)

- Glass: after the well, Kindling paints the Grove glyph (Violet) and the title `Grove` (Parchment). BIOS writes VGA `B8000` under the firmware flame; EFI maps GOP UC (same rite as the LAPIC window) and blits the 8×16. Kindling flame is first on GOP (firmware never ran there).
- `write` fd 1/2 also paints the glass. Serial ceremony unchanged. `flush` still `-EAGAIN`.
- `make below` += `grove` (G21). Suite-as-guest stays later (no Linux ABI, no keyboard).

## spawn-g20 (2026-10-05)

- `spawn` (house call 5) is real: named cairn spark, private pool copy (64 KiB law), own 64 KiB cup, own CR3 (clone of the kernel map, U/S only on spark+cup, kernel 2M supervisor, guard not-present). v0 replaces the light. ELF later.
- Second spark `wick` tells `second-leaf` (`the cup is its own`) then `exit(0)`. Feature `spawn-test` + `spawn-bin`. House-test: null spawn is `-EPERM`; a missing name is `-ENODEV` or `-ENOENT`.
- Tale/init stay on the shared all-U/S map. `grant`/`flush` still `-EAGAIN`. `make below` += `spawn` (G20).

## well-3g + font (2026-10-05)

- Firmware maps 2 MiB pages through 3 GiB (`PDPT[0..2]` at `0x3000` / `0xD000` / `0xE000`). `PDPT[3]` stays empty for the LAPIC window. 1025M and 2G used to `#PF` (`trap 14`) because one PD only covers 1 GiB.
- `probe_cap` walks live CR3 present-bits before the write/read; unmapped RAM stops the walk. A `#PF` during probe is a fail, not a well size.
- VGA 8×16 into plane 2 (`scripts/mkfont.py` → `fw/font8x16.bin`). Readback of `'*'` sets FMAP `0x8020`; miss prints `cerne-fw: no font`. Happy serial unchanged.
- `make audit` += 1025M (`well 1024 MiB`) + 2G (`well 2048 MiB`). `make below` += `font`.
- Doc truth: house light G15–G19 was on; Linux ELF / `spawn` still shut at that sitting. BOOT EFI spark line, syscalls header, SECURITY transcript, cairn `STOW` comment.

## stow-g2b (2026-10-01)

- Ink holds on iron: kernel ATA PIO read/write, `stow` call 9 (exact re-ink; sectors + LBA0 rewritten and ATA-verified, `-EIO` on mismatch). `tale` v2 stows the slate on boot1 (`stowed`), reads it `kept` on boot2. Sparks run from private pool copies (buf-in-cairn, twice learned). `make below` += `stow` (G19) green.

## tale-g2a (2026-10-01)

- The cairn (house voice throughout): leaves + sparks on disk (`docs/CAIRN.md`), `glean` call 8, `stow` 9 shut. Loader lays the cairn at `0x100000`; KMAP 0–16 untouched. First spark `tale` tells `first-leaf` at CPL3. `make below` += `tale` (G18) green.

## test-vm + timer-cpl3 (2026-09-30)

- `scripts/test-vm.sh`: throwaway QEMU VM (`happy|house|ring3|reclaim`, logs in `test-logs/`), 4× PASS.
- Timer from CPL3 proven: `init` sleeps 50 ms → `init slept` → `exit(0)` (6× IRQ0 in `-d int`); ring3/house gates require it. Virtual-wire via kernel `timer::wire()` (LAPIC EN + LINT0, readback FAIL 9–12); PIT/mask health FAIL 7/8; ISR-shared ticks volatile; `init`/`wire` split (window exists post-well only).

## reclaim-g1 (2026-09-30)

- G1 realms + reclaim: 4 KiB frame pool (LIFO, capped), realm guard + 64 KiB cup + PT split (2M → 4K, guard not-present), canary, drop restores + replays same frames. `kindling-reclaim.img` prints `kindling: reclaim ok`. `make below` += `reclaim` (G17) green. Per-realm CR3s wait on spawn (G4).

## ring3-init (2026-09-30)

- First Gleam light at CPL3: `gleam_init` (`write`/`yield`/`exit`) via `int 0xE0`; `kindling-ring3.img` prints `kindling: init ok` + `gleam exit 0`. GDT UCODE/UDATA + TSS (RSP0/IST1), pages U/S, PIT 100 Hz, #DF on IST1. `make below` += `ring3` (G16) green.

## house-v0 (2026-09-30)

- Gleam-only house calls first: `int 0xE0` gate, v0 `yield`/`exit`/`write(fd 1-2)`/`sleep`-stub/`time` + shut `spawn`/`grant`/`flush` (`-EAGAIN`); unknown → `-ENOSYS`. `docs/HOUSECALLS.md`.
- `kindling-house.img` prints `kindling: house ok`; happy path silent. `make below` += `house` (G15) green.

## sweep-phase1 (2026-09-30)

- Full sweep, phase 1 held: `audit ok` + `below ok` re-verified after fixes.
- Truth fixes: `docs/BOOT.md` spark line → `kindling: still only a spark`; dedup cup warning in `docs/FIRMWARE.md`; `.gitignore` covers `ld/*.bin` + `kindling*.img`.
- Lints green: `cargo fmt --check` clean; `cargo clippy -D warnings` clean (fw, limine, efi). `COM1 + 0` → `COM1`, collapsed nested `if let`.
- Upgrade audit: `uefi 0.35.0` / `limine 0.5.0` pinned — `uefi 0.41` deferred (EFI ABI churn, phase-1 freeze).
- Real disk chain (2026-09-28, uncommitted): firmware → `ld/cerne-ld` off disk → kernel, no `-device loader`; G13/G14 green.

## phase1-green (2026-09-24)

- Kindling phase 1 green: own page tables, bump well, cup-stack, CMOS probe,
  GDT/IDT, FMAP, trap names, ROM checksum. `make below` + `below-ten` pass.
- Firmware + fused loader are ours; Limine/OVMF borrowed at build, not vendored.
- Phase 2 (user ELF `write`/`exit`) shut. Syscall table all `missing` (ENOSYS).
