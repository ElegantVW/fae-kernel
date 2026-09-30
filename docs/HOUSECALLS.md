# House calls — Gleam's own gate

Gleam-only. Not Linux. Not POSIX. No foreign binaries.
Swift, purposeful, direct — no noise. Unknown refuses, never silently succeeds.

## Gate

- Vector `0xE0` (`int 0xE0`), DPL 3 (user may knock). Kernel `IDT 0xE0` → `vec_house`.
- Convention: `rax` = call number, `rdi/rsi/rdx` = args 0–2. Return in `rax`.
- Return: `0..i64::MAX` ok (bytes / ticks), negative `-errno` honest fail.
  Reuses Linux errno numbers for honesty, not compat: `38 ENOSYS` unknown,
  `1 EPERM` refused, `11 EAGAIN` not-ready-yet (stub, will grow).
- Clobbers `rcx/r11` (like `syscall`); preserves `rdi/rsi/rdx/r8/r9/r10`.
- Future: `syscall/sysret` for speed. `int 0xE0` first so the gate is simple
  and QEMU-provable. Same numbers survive the upgrade.

## First set (v0)

| # | Name | Args | Returns | Today |
|---|---|---|---|---|
| 0 | `yield` | — | `0` | done (co-op stub) |
| 1 | `exit` | `rdi` = code | never | done (`kindling: gleam exit N` + `hlt`) |
| 2 | `write` | `rdi` = fd, `rsi` = buf, `rdx` = len | bytes or `-errno` | done on fd 1/2 serial; other fd → `-EPERM`; len capped 1 MiB; null buf → `-EPERM` |
| 3 | `sleep` | `rdi` = ms | `0` | stub (no timer yet, returns at once — honest stub, not a lie about time) |
| 4 | `time` | — | `rdtsc` ticks | done (raw ticks, not ms — `ms` waits on timer) |
| 5 | `spawn` | — | `-EAGAIN` | shut (needs ELF + realms) |
| 6 | `grant` | — | `-EAGAIN` | shut (needs capabilities) |
| 7 | `flush` | — | `-EAGAIN` | shut (needs Lantern framebuffer) |

Anything else → `-ENOSYS`.

## Test

Feature `house-test` (like `trap6`): after the well+cup, the kernel knocks
from ring 0 — direct `dispatch()` + real `int 0xE0` — and prints
`kindling: house ok`. Happy path (no feature) prints nothing new;
the paved serial stays `cerne-fw / cerne-ld / fae-kernel / still only a spark / well …`.
