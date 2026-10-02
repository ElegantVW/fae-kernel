# fae-kernel changelog

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
