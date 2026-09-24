# fae-kernel

The kernel we write. The pink suite is userspace. This Arch box stays the daily house until a shell here is boringly reliable.

**Creature name: Kindling** — you light kindling, not the log. The lighting tools *are* the build tools (`make help`: flint, steel, tinder, hearth, kindle). Hearth the greeter is a different fire. First handoff signer is **Flint**, not Kindling.

Closed source for now. All rights reserved. No GitHub remote until Gil opens one (private is the default).

## What this is

x86_64. We write the column: **firmware (asm) → loader (fused) → kernel (Rust)**. Linux syscall ABI table is empty today. Userspace is the suite, later.

See [docs/BOOT.md](docs/BOOT.md), [docs/lore/kindling.md](docs/lore/kindling.md), [docs/handoff.md](docs/handoff.md). Name bowl: [docs/lore/fae-names.md](docs/lore/fae-names.md).

## Host

```text
nasm, qemu-system-x86_64, edk2-ovmf
rustup nightly (x86_64-unknown-none + x86_64-unknown-uefi) in ~/.cargo
xorriso (libisoburn) — only for the optional Limine ISO
```

On this box, pacman rust stays for the suite. Kernel builds with `~/.cargo/bin/cargo`.

```bash
cd ~/fae-kernel
make help           # how one lights Kindling
make kindle         # OUR firmware + kernel. No Limine. No OVMF.
make serial-uefi    # OVMF → our BOOTX64.EFI
make serial         # Limine ISO (optional crutch)
```

`limine/` is cloned at build time (`v10.x-binary`), not vendored.

## Layout

| Path | Role |
|---|---|
| `fw/cerne-fw.asm` | firmware + loader (reset vector) |
| `kernel/` | `no_std` crate (`start`, Limine bin, fw bin, EFI bin) |
| `limine.conf` | optional Limine menu |
| `docs/syscalls.md` | living Linux ABI table (honest stubs) |
| `docs/BOOT.md` | Limine vs UEFI, QEMU flags |

## Law

- We write the kernel. We do not copy Linux source. We do not ship an Arch ISO and call it ours.
- A missing syscall returns `ENOSYS`. Silent success is a lie.
- Offline. No telemetry.
- Not inside `~/faeOS` kit git.

## Phases

0 this tree (serial hello) → 1 memory → 2 user ELF `write`/`exit` → 3 fork/exec → … suite as guest.
