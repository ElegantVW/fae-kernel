# fae-kernel changelog

## phase1-green (2026-09-24)

- Kindling phase 1 green: own page tables, bump well, cup-stack, CMOS probe,
  GDT/IDT, FMAP, trap names, ROM checksum. `make below` + `below-ten` pass.
- Firmware + fused loader are ours; Limine/OVMF borrowed at build, not vendored.
- Phase 2 (user ELF `write`/`exit`) shut. Syscall table all `missing` (ENOSYS).
