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
