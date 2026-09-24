# Handoff — Kindling (fae-kernel)

New blocks **on top**. Do not rewrite history. **Every entry a different fae name** — never reuse, never `Korda` (that name is Vanguarda's Arch guest). All-ages. No PII, no passwords, no Kur walkthroughs.

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

## 2026-09-24 — Flint — struck the first spark

- Did: Column boots (our firmware + fused loader + `start()`). Native `BOOTX64.EFI`. Shared `start()`. Kindling named as the kernel fire; flint/steel/tinder/hearth/kindle are the lighting tools (`make help`). Handoff law + lore glossary (mana, crystals, casting). Breadcrumbs off the paved serial path (`strings`, panic prefix, `distclean`, unknown target). Fae-names dataset: `docs/lore/fae-names.md` (research; bowl starts Gleed, Aithinne, Ellyll…).
- Proof: `make kindle` / `make serial-fw` → `cerne-fw` / `cerne-ld` / `fae-kernel` / `kindling: still only a spark`. `make serial-uefi` → `cerne-efi` then the same kernel lines.
- Git: this tree, closed, no remote.
- Next: phase 1 memory (our page tables, a stack that is not `0x7000`). Debate the fae-names dataset when the researcher file lands.
- Do not: extra serial lines on the happy kindle path; push; claim a heap; fork Linux; reuse **Flint**.
