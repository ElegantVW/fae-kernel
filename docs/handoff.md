# Handoff — Kindling (fae-kernel)

New blocks **on top**. Do not rewrite history. **The fae name is the person or agent**, not the block — same worker, same name, every time. A new collaborator picks an unused name from the bowl and keeps it. Never `Korda` here (Vanguarda's Arch guest). All-ages. No PII, no passwords, no Kur walkthroughs.

Bowl of unused names: [lore/fae-names.md](lore/fae-names.md) (research dataset). Kernel fire-name: **Kindling**. First signer is not Kindling.

```
## YYYY-MM-DD — <Fae Name> — one-line spark

- Did:
- Proof:
- Git:
- Next:
- Do not:
```

---

## 2026-09-30 — Gleed — full sweep, phase-1 held (audit + lints + doc truth)

- Did: Full sweep, phase-1 only (no ELF / `write` / `exit`). Audit: `audit ok` + `below ok` re-verified; disk image re-checked (KMAP checksum, `'LDOK'` trailer, `'KNDL'` head). Diagnose: doc drift (`BOOT.md` spark line), duplicated cup warning (`FIRMWARE.md`), `.gitignore` missing `ld/*.bin` + `kindling*.img`, `cargo fmt --check` fail, `clippy -D warnings` fail on 3 bins. Fix (behaviour-preserving): `BOOT.md` → `kindling: still only a spark`; dedup cup warning; `.gitignore` += `/ld/cerne-ld.bin`, `/kindling.img`, `/kindling-*.img`; `start.rs` `COM1 + 0` → `COM1`; `cargo fmt`; collapse nested `if let` in `main.rs` + `efi_main.rs`. Improve: all 3 clippy targets clean now. Upgrade audit: `uefi 0.35.0` pinned vs `0.41.0` latest, `limine 0.5.0` crutch — deferred deliberately to keep phase-1 green (EFI ABI churn, no need). Intentional, not bugs: EFI `ram_end` capped `1<<30` (`efi_main.rs`) vs FW `0x1_0000_0000` (`mm.rs`) — EFI avoids PCI hole, FW probes CMOS; IDT 32–255 alias first-32 stubs (`idt.rs`) with PIC masked; `nasm -w+all` abs-reloc warnings are flat-ROM `default abs` by design.
- Proof: `sh scripts/audit-kindle.sh` → `kindling: audit ok` (256M/8M/4M/1G/none/wrong/lilac/q35). `sh scripts/below.sh` → `kindling: below ok` (audit+trap6+fmap-bad+fw-trap+kmap-bad+efi) after fixes. `cargo fmt --check` clean; `cargo clippy -D warnings` clean for `fae-kernel` + `fae-kernel-fw --features fw-link` + `cerne-efi --features efi`. Happy serial still `cerne-fw / cerne-ld / fae-kernel / kindling: still only a spark / well 256 MiB`.
- Git: this tree, local. Modified: `.gitignore`, `docs/BOOT.md`, `docs/FIRMWARE.md`, `kernel/src/{start,main,efi_main,fw_main}.rs`. Pre-existing dirty (seal 2026-09-28 real-chain: `GNUmakefile`, `README`, `docs/*`, `fw/*`, `scripts/*`, untracked `ld/`, `scripts/mkimg.py`) still uncommitted — not pushed, per law. `kindling*.img` now ignored, no longer noise.
- Next: commit seal+Gleed as one phase-1 checkpoint when Gil says so; then phase 2 only on Gil's word. Optional later: `scripts/*` surface errors instead of `>/dev/null` on `steel`; `cargo audit` / `cargo outdated` gate when network allowed.
- Do not: open phase 2 (ELF / syscalls); bump `uefi`/`limine` majors to chase latest; unify EFI/FW caps without QEMU-iron proof; push.

---

## 2026-09-28 — seal — the loader stands alone (real disk boot)

- Did: Un-fused the loader. `fw/cerne-fw.asm` now reads **LBA 1..32** of the boot disk to `0x9000` (ATA PIO, SRST retries, floating-bus check) and trusts it only if the slot trailer `'LDOK'` landed at `0xCFFC`; then it hands over in long mode and jumps `0x9000`. `ld/cerne-ld.asm` (new — self-contained: own ATA PIO, own serial) reads the KMAP (disk LBA 0), loads the kernel straight to `0x200000` in 128-sector commands, checks its XOR **and** `'KNDL'`, jumps `0x200004`. `scripts/mkimg.py` casts `kindling.img` (KMAP · 16KiB loader slot · kernel at LBA 33) with `--bad` / `--wrong` for the gates. **`-device loader` is gone from every bowl** — nothing is cheated into RAM any more. KMAP + loader contract in `docs/FIRMWARE.md`; gates **G13** (real chain) and **G14** (bad KMAP refuses) in `docs/BELOW.md`. q35 needs `-device piix3-ide` for real ports behind `0x1F0`; pc already has them.
- Proof: `make below` → `kindling: below ok` (256M 8M 4M 1G none wrong lilac q35 trap6 fmap-bad fw-trap **kmap-bad** efi). `make below-ten` → `kindling: below ten ok`. Happy path byte-for-byte the same transcript: `cerne-fw` / `cerne-ld` / `fae-kernel` / `kindling: still only a spark` / `kindling: well 256 MiB`.
- Git: this tree, local. Not pushed — Flint's `Do not: push` still stands.
- Next: a write path and a filesystem when the guest needs them; phase 2 (user ELF) still waits on Gil's word. Note the stop-line Flint drew (`Do not: … disk`) has moved: the real chain was asked for by name, and that call is Gil's — logged here so the line moves visibly and not by accident.
- Do not: put the loader back in the ROM; jump without `'LDOK'` + `'KNDL'` + kernel XOR all three agreeing; leave a failure path that half-jumps.

---

## 2026-09-24 — Flint — firmware contract, 4K low map, ROM sum, FMAP regions

- Did: `docs/FIRMWARE.md`. IBM ROM checksum (last byte, `scripts/romsum.py`). FMAP v1: nreg=2, `0–0x9F000` and `1MiB–ram_end`. First 2 MiB as 4 KiB pages (PCD+PWT on `A0000–FFFFF`). Traps print a number (`cerne-fw: trap N`). Kernel checksum is `ram xor nreg xor KNDL`.
- Proof: `make below` → `kindling: below ok`.
- Git: this tree.
- Next: firmware is at the stop-line unless Gil wants more ROM. Phase 2 still shut.
- Do not: ACPI; disk; ELF; push.

## 2026-09-24 — Flint — firmware IDT, VGA mode 3, PIC ICW

- Did: ROM sets 80×25 by VGA registers (no `int 10h`). PIC ICW1–4 then mask. Real-mode IVT 0–31, 32-bit IDT at `0x5000`, 64-bit IDT at `0x6000` until Kindling `lidt` — all say `cerne-fw: trap`. 64-bit `lgdt`. NASM sections, 65536 bytes, no warning. `AUDIT_FW_TRAP` in `make below`. Kernel untouched.
- Proof: `make below` → `kindling: below ok` (includes `ok fw-trap`). `make below-ten` → `kindling: below ten ok`.
- Git: this tree.
- Next: still firmware if Gil wants; phase 2 still shut.
- Do not: ELF; push.

## 2026-09-24 — Flint — below gate

- Did: `docs/BELOW.md`. FPU/SSE (`fninit` + `movaps`). PIC masked in firmware and kernel. FMAP checksum (`ram xor KNDL`). `ud2` → `trap 6`. A20 only if off. `make below` = audit + trap + fmap-bad + efi. Limine stays a crutch. **No phase 2.**
- Proof: `make below` → `kindling: below ok`. `make below-ten` → `kindling: below ten ok` (fmap-bad no longer poisons the next flint).
- Git: this tree.
- Next: the below gate is green ten times. Phase 2 only when Gil says the spark is enough.
- Do not: user ELF; `write`; `exit`; push.

## 2026-09-24 — Flint — GDT, named traps, probe, EFI well

- Did: Own GDT on the BIOS path. IDT vectors 0–31 print `kindling: trap N`. CMOS well is probed with write/read. EFI: trust the map, CLI before LIDT, **switch cup before CR3** (OVMF stack is not in our identity map). LoadedImage so we do not pour tables on the image.
- Proof: `make audit` ok. `make serial-uefi` → `efi map 229 MiB` then `well 228 MiB · stack 64 KiB cup`.
- Git: this tree.
- Next: phase 2 ELF.
- Do not: 5-level paging well; Limine well; push.

## 2026-09-24 — Flint — audit and thicken the spark

- Did: CMOS well drives firmware page count (no fake 1 GiB). IDT so a trap says `kindling: trap` instead of a silent reset. Cup canary + 16-byte stack. `make audit` (256/8/4/1G/none/wrong/lilac/q35).
- Proof: `make audit` → `kindling: audit ok`.
- Git: this tree.
- Next: phase 2 ELF.
- Do not: claim a real #PF recover; push.

## 2026-09-24 — Flint — lilac truth

- Did: First glyph is Lilac (VGA DAC index 13 + serial `ESC[95m`). CMOS well (we are the BIOS; no `int 0x15`). `KNDL` guest check. Honest MiB / `the well ran dry` / `no guest at 0x200000`. Grove / Gleam / Kindling + 16-color identity (`docs/identity/`).
- Proof: 256M → well 256; 8M → well 8; 4M → dry; 1G → 1024; no/wrong kernel → no guest. Serial starts `ESC[95m`.
- Git: this tree.
- Next: phase 2 ELF. Debate Grove/Gleam names.
- Do not: claim E820; push.

## 2026-09-24 — Flint — Kindling takes the well

- Did: Phase 1 memory on the kindle path. Own identity page tables (2 MiB pages), bump well, 64 KiB cup-stack (not firmware `0x7000`). Handoff names the **worker**, not the block — Flint stays Flint.
- Proof: `make kindle` → rungs, then `kindling: well 256 MiB · stack 64 KiB cup`.
- Git: this tree, closed.
- Next: phase 2 user ELF `write` / `exit`. EFI/Limine still spark-only (no well yet).
- Do not: extra eggs on the paved serial; push; 4K guard pages yet (canary only).

## 2026-09-24 — Flint — struck the first spark

- Did: Column boots (our firmware + fused loader + `start()`). Native `BOOTX64.EFI`. Shared `start()`. Kindling named as the kernel fire; flint/steel/tinder/hearth/kindle are the lighting tools (`make help`). Handoff law + lore glossary (mana, crystals, casting). Breadcrumbs off the paved serial path (`strings`, panic prefix, `distclean`, unknown target). Fae-names dataset: `docs/lore/fae-names.md` (research; bowl starts Gleed, Aithinne, Ellyll…).
- Proof: `make kindle` / `make serial-fw` → `cerne-fw` / `cerne-ld` / `fae-kernel` / `kindling: still only a spark`. `make serial-uefi` → `cerne-efi` then the same kernel lines.
- Git: this tree, closed, no remote.
- Next: phase 1 memory (our page tables, a stack that is not `0x7000`). Debate the fae-names dataset when the researcher file lands.
- Do not: extra serial lines on the happy kindle path; push; claim a heap; fork Linux. **Flint** keeps this name on later blocks.
