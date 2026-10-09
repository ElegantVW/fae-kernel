# House calls — Gleam's own gate

Gleam-only. Not Linux. Not POSIX. No foreign binaries.
Swift, purposeful, direct — no noise. Unknown refuses, never silently succeeds.

## Gate

- Vector `0xE0` (`int 0xE0`), DPL 3 (user may knock). Kernel `IDT 0xE0` → `vec_house`.
- Convention: `rax` = call number, `rdi/rsi/rdx` = args 0–2. Return in `rax`.
- Return: `0..i64::MAX` ok (bytes / ticks), negative `-errno` honest fail.
  Reuses Linux errno numbers for honesty, not compat: `38 ENOSYS` unknown,
  `1 EPERM` refused, `11 EAGAIN` not-ready-yet (`grant`/`flush` still shut;
  nested `spawn` while a spark is already lit).
- Clobbers `rcx/r11` (like `syscall`); preserves `rdi/rsi/rdx/r8/r9/r10`.
- Future: `syscall/sysret` for speed. `int 0xE0` first so the gate is simple
  and QEMU-provable. Same numbers survive the upgrade.

## Calls

| # | Name | Args | Returns | Today |
|---|---|---|---|---|
| 0 | `yield` | — | `0` | done (co-op stub) |
| 1 | `exit` | `rdi` = code | never | done: smoors into the Light (the Light's `spawn` receives this last word in `rax`); with no Light, serial **and glass** `kindling: gleam exit N` + `hlt` |
| 2 | `write` | `rdi` = fd, `rsi` = buf, `rdx` = len | bytes or `-errno` | done on fd 1/2 serial **and** the glass (VGA text / GOP blit); other fd → `-EPERM`; len capped 1 MiB; null buf → `-EPERM` |
| 3 | `sleep` | `rdi` = ms | `0` | done: TSC deadline after the well (HPET or polled PIT). BIOS may `hlt` on IRQ0. Dead clock returns at once (`no tick`) |
| 4 | `time` | — | ms | done: milliseconds from calibrated TSC when the clock is live; PIT ticks if TSC never armed; raw `rdtsc` only when both are dark |
| 5 | `spawn` | `rdi` = name | last word or `-errno` | done (named spark onto private pages + own cup + own CR3; cairn first, then a live FAT volume's **root** by the same Gleam name). Kindles, then returns the spark's last word when it smoors. One spark at a time — a live Light → `-EAGAIN`. Guest `#UD`/`#PF`/`#GP` smoors with `-EIO` (`the spark went out`); never resumes at the fault RIP. null/empty/slashy/oversize → `-EPERM`; empty or bigger than 64 KiB → `-EPERM`; no cairn and no volume → `-ENODEV`; missing → `-ENOENT`; bad chain → `-EIO`. Flat `nasm -f bin` first; ELF later. |
| 6 | `grant` | — | `-EAGAIN` | shut (needs capabilities) |
| 7 | `flush` | — | `-EAGAIN` | shut (needs Lantern framebuffer) |
| 8 | `glean` | `rdi` = name, `rsi` = buf, `rdx` = len | bytes or `-errno` | done (cairn leaves first; book names `hands`/`twin`/`hand` then KINDLOG when live; other names, if missing from the cairn, the FAT volume's **root** by the same Gleam name, 1–64, no `/`. FAT is the medium for those — no 8.3 in the house call. LFN is exact case and wins (`leaf` is not the 8.3 `LEAF` page). 8.3-only aliases fold. Missing → `-ENOENT`; no cairn and no volume → `-ENODEV`; bad xor / bad chain → `-EIO`; bad args → `-EPERM`. Cap 1 MiB. Read only.) |
| 9 | `stow` | `rdi` = name, `rsi` = buf, `rdx` = len (= `datalen`) | bytes or `-errno` | done (cairn leaves first, exact measure, ATA re-read + LBA0; book names then KINDLOG when live; other names, if missing from the cairn, the FAT volume's **root** — WRITE(10) each sector, READ(10) compare, KINDLING `85C7-AA81` only. FAT32 creates a missing or empty root file of `len` (LFN + 8.3). Present wrong non-zero size → `-EPERM` (glass `the leaf is the wrong measure`). FAT16 re-inks only (`-ENOENT` if missing). Wrong volume → `-EPERM`; no cairn and no volume → `-ENODEV`; verify / no cluster / dir full → `-EIO`. Ink is for leaves — sparks refuse. Cap 1 MiB.) |
| 10 | `read` | `rdi` = fd, `rsi` = buf, `rdx` = len | bytes or `-errno` | done on fd 0 (PS/2 ASCII **or** USB HID boot keyboard); other fd → `-EPERM`; neither 8042 nor xHCI kbd → `-EAGAIN`; blocks until a byte when live. Polls the 8042 data port and the xHCI event ring; `hlt` only when the PIT is armed (EFI has no IRQ0, iron IRQ1 often dies after ExitBootServices). USB boot kbd live → 8042 scancodes are drained, not pushed (same laptop key must not land twice). A glyph is held until that key is up. Dark — the spark `write`s if the glyph should show (`keeper` echoes the name; the word echoes as `*`; glass `0x08` is backspace) |
| 11 | `roll` | `rdi` = buf, `rsi` = len (≥ 256) | count or `-errno` | done. Copies occupied keeper **names** (32-byte records, never verifiers) after unsealing `hands` and `twin` and comparing them. Empty / wax / a lone leaf → `0`. Two live seals that disagree → glass `the book is split`, `-EIO`. AES-NI or RDRAND miss → `the book stays shut`, `-ENODEV` |
| 12 | `enlist` | `rdi` = name, `rsi` = word, `rdx` = confirm (NUL-terminated) | `0` or `-errno` | done. Confirm mismatch / empty → `-EINVAL` (no glass; `keeper` compares first and writes `the second word is a stranger`). Duplicate → `that hand is taken`, `-EEXIST`. Full (8) → `the book is full`, `-EPERM`. Seals both leaves with fresh nonces, inks `hand`. A stow miss waxes both leaves so the next `roll` is empty. KINDLING `85C7-AA81` on KINDLOG (fixed LBAs; FAT is the ESP). Spark then writes `the name is cut` / `the word sleeps in the twin` |
| 13 | `choose` | `rdi` = name, `rsi` = word | `0` or `-errno` | done. Verifier match inks `hand` and returns 0. Spark writes `this fire knows you`. Miss → `the word fails`, `-EPERM`. Split refuses |
| 14 | `dismiss` | `rdi` = name, `rsi` = word | `0` or `-errno` | done. Last live keeper → `the last hand stays`, `-EPERM`. Wrong word → `the word fails`. Clears the slot and reseals both leaves. Spark writes `that name is given back` |

Anything else → `-ENOSYS`. Wrapping keys never leave the kernel. There is no generic cipher house call this sitting.

## Ring-3 init (first Gleam light)

`gleam_init` (in-kernel, CPL3): `write` → `yield` → `exit(0)` via `int 0xE0`.
Entry via `iretq` (`SS/RSP/RFLAGS/CS/RIP` = `0x23` / private 16 KiB stack /
`0x202` / `0x1B` / `gleam_init`). GDT carries UCODE `0x18` / UDATA `0x20` (DPL 3)
+ TSS `0x28` (RSP0 = kernel cup, IST1 = double-fault stack). Tale/init still
ride the shared all-U/S map; `spawn` clones CR3, strips U/S from kernel 2M
leaves, and grants user only the spark + its cup (guard not-present, APIC
supervisor). PIT IRQ0 ticks (100 Hz) via LAPIC virtual-wire
(EN + LINT0 ExtINT, programmed post-well where the window is mapped and
verified by readback — a dead timer fails the self-test instead of hanging);
#DF rides IST1. `init` proves the whole path: `write` → `yield` → `sleep 50`
→ `time` advanced → `init slept` → `exit(0)`. Two inline-asm laws
this gate paid for: entry addresses ride fixed regs (never `in(reg)`, which
may pick RSP), and symbol addresses load via `lea` (there is no push-imm64 —
`push {sym}` pushes the qword AT the symbol).

## Test

Feature `house-test` (like `trap6`): after the well+cup, the kernel knocks
from ring 0 — direct `dispatch()` + real `int 0xE0` — and prints
`kindling: house ok`. Null `spawn` must be `-EPERM`; a missing name is
`-ENODEV` or `-ENOENT`. Feature `spawn-test` then looses `ember` on a
private CR3: `kindling: spawn ok`, ember kindles `wick`, the second-leaf,
`stayed`, `gleam exit 0`. Packed with `splanc` instead, ember kindles that
`ud2`: `kindling: trap 6`, `the spark went out`, `gleam exit 1` — `stayed`
is a lie.
Feature `ingle-test` looses `ingle`: `kindling: ingle ok`, the spark writes
`ingle`, kindles `keeper` when that spark is present, greets an inked `hand`
after last word 0, `read` waits, Enter yields `the fire is lit`, then `ingle`
kindles `leaf` once (a miss is quiet). A last word ≥ 0 writes `the light remains`.
`read` until `q`, then `gleam exit 0`. A miss of `keeper` is quiet. A last
word other than 0 does not light the fire.
Feature `leaf-test` looses `leaf`: `kindling: leaf ok`, the spark gleans
`LEAF`, writes the page, `read` waits for `q`, then `gleam exit 0`.
Feature `tale-test` on EFI (`efi-stow`) looses `tale` against a FAT slate:
`kindling: tale ok`, first boot `stowed` / `ink holds`, second boot of the
same usb-storage image `kept` / `ink holds` (never `stowed`). The volume
serial is KINDLING `85C7-AA81`. Cairn has `tale` + `first-leaf`; the slate
lives on the volume root.
Happy path (no `*-test` feature) lights `ingle` when the cairn has it
(`kindling: ingle ok`, then the spark). Missing cairn is a quiet halt.
The paved serial still prints `cerne-fw / cerne-ld / fae-kernel / still only a spark / well …` first.
