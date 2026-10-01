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

## 2026-09-30 — Gleed — test-vm + timer from CPL3 (virtual-wire, init slept)

- Did: `scripts/test-vm.sh` — the throwaway VM (`happy|house|ring3|reclaim` or any image + wants; pc/256M/serial-to-`test-logs/`, PASS/FAIL, exit 1 on MISS). All four cases green. Then the untested path: `init` now sleeps 50 ms and proves a tick passed (`init slept`, else `no tick` + `exit(1)`); ring3/house gates require it. Hunt, honestly logged: sleep hung, `-d int` showed zero IRQ0 while CPL3 house calls flowed; PIT latch test proved the counter runs and the mask is open, so the break sat between PIC and CPU; monitor `info pic/lapic` proved it (`irr=01 imr=fe` stuck, LVT0 masked, APIC disabled) — our column never enabled the LAPIC (SeaBIOS-alike boards do). Fix where it is verifiable: kernel `timer::wire()` post-well (APIC window mapped in our tables) sets EN + SVR + LINT0 ExtINT with readback (FAIL 9/10/11/12 instead of a hang); PIT health + mask checks are FAIL 7/8. A real-mode firmware attempt came out again (writes vanish while disabled) and was reverted, not kept as dead MMIO. Also fixed along the way: ISR-shared `TICKS` now volatile both ways; `init`/`wire` split (init ran pre-well, before the window exists — found via the stash reading zero); `test-logs/` ignored.
- Proof: `test-vm happy|house|ring3|reclaim` → 4× PASS. Ring3 transcript now `… / init ok / init slept / gleam exit 0` with 6× `v=20` in `-d int`. `below ok` (ring3 step requires `init slept`).
- Git: this tree, local. Added `scripts/test-vm.sh`; modified `kernel/src/{house,timer,mm,start}.rs`, `fw/cerne-fw.asm` (net zero — attempt reverted), `scripts/below.sh`, `.gitignore`, `docs/HOUSECALLS.md`.
- Next: G2 Seal files (ATA write → house FS → file calls); realm cups execute on spawn (G4).
- Do not: program the LAPIC before its window exists; trust a timer without readback; let `in(reg)` near `push rsp`.

---

## 2026-09-30 — Gleed — G1 realms + reclaim (frame pool, guard, G17)

- Did: G1 foundation. `mm.rs`: 4 KiB frame pool (LIFO, capped 4096, `pool_init` from bump high-water), `Realm` (guard + 16 cup pages + PT page; splits the covering 2M PD entry into 512×4K with guard not-present; canary at cup base; `drop` verifies canary, restores the 2M entry, CR3-flush, hands pages back in exact reverse). `reclaim-test` feature + `reclaim-bin`: create → guard shut → drop → count whole → re-create replays same frames → guard shut → drop → count whole → `kindling: reclaim ok`. Happy path silent (pool init only). Per-realm CR3s deferred to spawn (G4) — said here, not silently dropped. Pool cap documented in code (`pool_count` tells).
- Proof: `kindling-reclaim.img` boots `… / house ok / kindling: reclaim ok / well 256 MiB`. `sh scripts/below.sh` → `below ok` (audit+trap6+house+ring3+**reclaim**+fmap-bad+fw-trap+kmap-bad+efi). `fmt` + `clippy -D warnings` clean (all six targets). Guard proven by entry bit, not by fault (trap recovery is out-of-gate — touching the guard would `hlt`, honestly).
- Git: this tree, local. Modified/added: `kernel/src/mm.rs`, `kernel/src/start.rs`, `kernel/{Cargo.toml,GNUmakefile}`, `scripts/below.sh`, `.gitignore`, `docs/{BELOW,README}.md`.
- Next: G2 Seal files (ATA write → house FS → file calls); realm cups execute on spawn (G4).
- Do not: hand pool pages out of LIFO order and expect replay; split an already-split PD (oom is the honest answer); map the guard.

---

## 2026-09-30 — Gleed — ring-3 init (first Gleam light, G16)

- Did: CPL3 `gleam_init` speaking only house calls (`write` → `yield` → `exit(0)` via `int 0xE0`). `gdt.rs`: UCODE `0x18` / UDATA `0x20` (DPL 3) + TSS `0x28` (RSP0 = cup top, IST1 = 8 KiB #DF stack, `ltr`). `mm.rs`: all pages U/S while realms shut (PML4 + PDPT + 2M PD). `timer.rs` (new): PIT 100 Hz, IRQ0 unmasked on BIOS only; `sleep` blocks / `time` = ms there, stub + `rdtsc` fallback on EFI/crutch. `idt.rs`: `vec_timer` (`0x20`, EOI), #DF on IST1. `house.rs`: `enter_init` via `iretq` (fixed-reg frame, `LEA` entry). `below.sh` += `ring3` step (G16). Two asm laws paid for and written into `HOUSECALLS.md`: fixed regs for addresses (never `in(reg)` for the pushed RSP), `lea` for symbol addresses (no push-imm64).
- Proof: `kindling-ring3.img` boots `cerne-fw / cerne-ld / fae-kernel / still only a spark / ok / house ok / well 256 MiB / kindling: init ok / kindling: gleam exit 0`. `sh scripts/below.sh` → `below ok` (audit+trap6+house+**ring3**+fmap-bad+fw-trap+kmap-bad+efi). `fmt` + `clippy -D warnings` clean (fw, house, ring3, limine, efi). QEMU `-d int` proved the two faults on the way: `in(reg)` picking RSP (frame garbage) and `push {sym}` pushing code bytes.
- Git: this tree, local. Modified/added: `kernel/src/{gdt,mm,timer,idt,house,start,main,fw_main,efi_main}.rs`, `kernel/{Cargo.toml,GNUmakefile}`, `scripts/below.sh`, `.gitignore`, `docs/{HOUSECALLS,BELOW,README}.md`.
- Next: timer-proof `sleep` test from CPL3; `syscall/sysret` for speed (numbers survive); realms/grant (per-address U/S); FS write + exec-from-disk; Lantern flush.
- Do not: map user pages supervisor-only again; let `in(reg)` near `push rsp`; trust `push {sym}` for an address.

---

## 2026-09-30 — Gleed — house-call first (Gleam gate v0, `int 0xE0`)

- Did: Gleam-only house calls, house-call first. New `docs/HOUSECALLS.md` (gate `0xE0` DPL 3, `rax`=n `rdi/rsi/rdx`=args, `-errno` honesty: `38` unknown / `1` refused / `11` not-ready). New `kernel/src/house.rs` (`yield` 0 / `exit` 1 / `write` 2 on fd 1-2 serial capped 1 MiB / `sleep` 3 stub / `time` 4 `rdtsc` + shut `spawn` 5 / `grant` 6 / `flush` 7 → `-EAGAIN`). `idt.rs`: `vec_house` stub (preserve, shuffle to SysV, `call house_entry`, `iretq`), `0xE0` earns `0xEE` DPL-3 gate (no trap alias). `house-test` feature + `house-bin` target: direct `dispatch()` + real `int 0xE0` knock, prints `kindling: house ok`; happy path silent (paved serial unchanged). Gate fortified: `below.sh` += `house` step (G15), `.gitignore` += `kernel.house.bin`, `BELOW.md` G15, `README` house row.
- Proof: `make -C kernel house-bin` + `kindling-house.img` boots `cerne-fw / cerne-ld / fae-kernel / still only a spark / ok / kindling: house ok / well 256 MiB`. `sh scripts/below.sh` → `kindling: below ok` (audit+trap6+**house**+fmap-bad+fw-trap+kmap-bad+efi). `cargo fmt --check` clean; `clippy -D warnings` clean (fw, fw+house-test, limine, efi).
- Git: this tree, local. Modified: `docs/HOUSECALLS.md` (new), `kernel/src/{house,start,idt,main,fw_main,efi_main}.rs`, `kernel/{Cargo.toml,GNUmakefile}`, `scripts/below.sh`, `.gitignore`, `docs/{BELOW,README}.md`.
- Next: ring-3 `init` (first Gleam light) + TSS/IST; timer so `sleep`/`time` mean ms; `syscall/sysret` for speed (numbers survive). Then realms/grant, FS write, Lantern flush.
- Do not: speak Linux on this gate; add eggs to the paved serial; let `spawn`/`grant`/`flush` silently succeed (`-EAGAIN` until real).

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
