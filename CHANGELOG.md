# fae-kernel changelog

## usb-msc-g29f (2026-10-06)

- Iron G29e printed `no desc`: reset and Address Device lived, GET_DESCRIPTOR did not. EP0 control TDs now match xHCI 1.0 / Linux: TRT IN=3 OUT=2, Setup/Data/Status are separate TDs (no Chain), first TRB cycle held until the rest are written, `mfence` before the doorbell. Average TRB Length 8 on EP0. A failed control resets EP0 and sets the dequeue pointer; GET_DESCRIPTOR retries, then Evaluate 64/8 if PSI was a lie. QEMU hid the swapped TRT and the Chain bit.

## usb-msc-g29e (2026-10-06)

- Iron G29d printed `no rst`. PED (PORTSC bit 1) is RW1CS: writing 1 *disables* the port. `port_neutral` had copied Linux RO but included PED; after reset the ack wrote PED=1 and killed Enable. QEMU ignores that write. Neutral now matches Linux (CCS/OC/speed/PP/PLS only). USB2 waits for PR to self-clear; USB3 uses WPR. Never write 1 to PED unless we mean to disable.

## usb-msc-g29d (2026-10-06)

- Iron G29c was a blink at `well` then still `no dev` (CCS, no GET_DESCRIPTOR). PORTSC writes now match Linux (preserve RO/RWS, do not write-1 all change bits with PR). USBLEGSUP waits up to 1 s for BIOS to drop the semaphore. Glass splits the miss: `no rst` / `no addr` / `no desc` / `no bot` / `no cap`. GET_DESCRIPTOR 18 still runs if the 8-byte probe stalls.

## usb-msc-g29c (2026-10-06)

- Iron G29b printed `no dev` after a minute at `well`. CCS was seen, GET_DESCRIPTOR never was: failed PORTSC resets retried with 4 million uncached MMIO polls, and USBLEGSUP/PP/HCRST used the same spin. Waits are milliseconds on the G28 clock (20–100 ms). One reset try per port; a second scan 100 ms later catches a late stick. Dead BAR (`0xFFFFFFFF`, caplen>0x80) is refused. PCI CF8 walks buses 0..=15. CAS gets a warm reset. EP0 starts at 8 for FS/LS, then Evaluate. IMAN IE at runtime 0x20. Glass still `no ccs` / `no dev` / `no bot` / `no cap` / `msc`.

## usb-msc-g29b (2026-10-06)

- Iron G29 printed `no msc` (and `no usb`): bringup never kept a host. QEMU hid it — devices CCS immediately. After HCRST the IdeaPad camera/BT can CCS first; `power_ports` returned on that and the side-port Databar (USB2 HS, BOT 8/6/50) was still reconnecting. Settle 150 ms on the G28 clock, then rescan (100 ms × 4) so a late CCS is seen. USB3 ports warm-reset when !PED. 10 ms reset recovery before Address/GET_DESCRIPTOR. Glass names the miss: `no ccs` / `no dev` / `no bot` / `no cap` / `msc`. Empty Thunderbolt xHCI bails after one extra settle.

## usb-msc-g29 (2026-10-06)

- xHCI now walks every root port and one hub hop for BOT mass storage (class 8 / subclass 6 / proto `0x50`) as well as a HID boot keyboard. HID no longer stops the scan. Bulk IN/OUT, CBW/CSW, TEST UNIT READY retries, READ CAPACITY(10). Glass `msc` / `no msc` after `tick`. HID interrupt completions stay off the BOT `xfer_seen` flag so ingle still hears Enter. `make below` += efi-msc (`usb-kbd` on `xhci.0` port 1, `usb-storage` on port 2). Stow is still ATA; FAT/KINDLOG waits.

## clock-g28 (2026-10-06)

- House `sleep`/`time` were path-dependent: PIT ms on BIOS, a no-op + raw `rdtsc` on EFI. After the well, Kindling maps HPET (`0xFED00000` in the `0xFEC00000` 2M UC leaf beside the LAPIC), else polls the PIT, calibrates TSC, and `time` is milliseconds on every path that arms. Glass `tick` / `no tick`. EFI proves `sleep 50` with `kindling: efi slept`. `make below` += efi-sleep. BIOS `init slept` stays.

## usb-hub-g27b (2026-10-06)

- Iron G27 printed `no usb` then `ingle`. QEMU hid it: `qemu-xhci` has no PPC and the kbd sits on a root port. Intel wants PP on every port before CCS, Enable Slot type from Supported Protocol (xECP **Next** is a DWORD offset from this cap, not the BAR), and the IdeaPad keys often hang behind one USB2 hub. Glass now says `xhci` then `usb kbd` or `no usb` (`no xhci` if PCI missed). `map_uc` may replace a present WB 2M leaf. HID match is class 3 proto 1. OVMF + hub can park the PE at the top of a 256M bowl; the bump stays at 4 MiB when after-image has no cup. `make below` += efi-usb-hub (`usb-hub` on `xhci.0` port 1, `usb-kbd` on `1.1`).

## usb-hid-g27 (2026-10-06)

- Iron G26 showed `ingle` waiting; the IdeaPad keyboard is USB. House `read` now polls xHCI as well as the 8042. PCI CF8 finds class `0x0C0330`, maps the BAR UC, takes USBLEGSUP, halt/reset/run, addresses a HID boot keyboard, `SET_PROTOCOL` 0. Reports become ASCII on the same queue. Serial `kindling: xhci 0xBAR` and `kindling: usb kbd` (or `no usb`). `make below` += efi-usb (`qemu-xhci` + `usb-kbd`; must print `usb kbd` before `sendkey`). 8042 `efi-ingle` stays.

## glass-g26 (2026-10-05)

- Stick was current (md5 match); iron still Grove + quiet cursor. EFI well identity-mapped only 0..1 GiB, then `mov cr3` — Insyde PE/GOP above that blinds the panel, QEMU OVMF stays low and serial-only gates stay green. `prepare_well` now maps the loaded image (WB) and GOP (UC) into the new tables before CR3. If the PE ends past the well cap, the bump drinks 4 MiB (or after a low image), never past `ram_end`. `show()` writes `kindling` on the log row (pre-EBS GOP already works on iron). After CR3 the glass says `well`. ConOut prints `pe` / `gop` hex. Serial `kindling: gop 0xADDR 0xPITCH` after the well. `make below` += efi-glass (QEMU `xp` of the log row) and stick (`cmp` KINDLING when the pen is plugged; skip if the pen is out or sudo is missing). Audit 2G bowl waits 8s.

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
