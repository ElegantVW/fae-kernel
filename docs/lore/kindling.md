# Kindling — the kernel's fire-name

**Kindling** is the proposed creature name of this kernel (repo still `fae-kernel`). Not Hearth (greeter). Not Cerne (older proposal, kept as firmware file names for now). Not Pixie.

You do not light a log. You light **kindling**. The tools that light it *are* the tools that reach `start()`.

| To light a fire | In this house | Make / command |
|---|---|---|
| **Flint** | `nasm` — strikes the first spark (asm) | `make flint` |
| **Steel** | `cargo` / `rustc` nightly — the striker | `make steel` |
| **Tinder** | firmware ROM `fw/cerne-fw.bin` — catches | `make tinder` |
| **Breath** | serial (`-serial stdio`) — air on the coal | always on `make hearth` / `kindle` |
| **Hearth** | QEMU — the bowl we dare to burn in | `make hearth` |
| **Bellows** | `make` itself — air in rhythm | `make help` |
| **Kindling** | the kernel flat / EFI | `make steel` then jump |
| **Borrowed match** | Limine — we can light without it | `make serial` (crutch) |

`make kindle` = flint + steel + hearth: our firmware, our kernel, our QEMU. The paved fire.

Ceremony on the serial (do not add eggs here):

```
cerne-fw
cerne-ld
fae-kernel
kindling: still only a spark
```
