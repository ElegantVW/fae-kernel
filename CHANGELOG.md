# fae-kernel changelog

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
