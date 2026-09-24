# fae-kernel

The kernel we write. The pink suite is userspace. This Arch box stays the daily house until a shell here is boringly reliable.

**Proposed creature name: Cerne** (heartwood — the living core of the tree). Vacant until Gil says so. Calcifer is the fire in the *hearth*; Hearth is already the greeter. Cerne is the fire *in the wood*, without copying Miyazaki.

Closed source for now. All rights reserved. No GitHub remote until Gil opens one (private is the default).

## What this is

x86_64 kernel, Rust + tiny `asm`, **Limine** boot protocol, Linux syscall ABI (empty table today). First demo: QEMU prints `fae-kernel` on serial.

## Boot: Limine vs raw UEFI

**UEFI** is the firmware on the machine (or OVMF in QEMU). Speaking UEFI ourselves means we are an `.efi` app: memory map, GOP framebuffer, exit boot services, then our own page tables. More spec, more code, one less extra binary.

**Limine** is a small bootloader. Firmware loads Limine; Limine drops us in long mode at `kmain` with a memory map (and a framebuffer if we ask). We depend on the Limine binary; we do not write the firmware handshake.

This repo uses **Limine** so phase 0 is a serial line, not a year of UEFI. The ISO still boots on BIOS *and* UEFI firmware (Limine ships both). Raw UEFI stub can come later if we want to drop the bootloader.

## Host

```text
rustup (nightly + x86_64-unknown-none)   # ~/.cargo — does not replace pacman rust
qemu-system-x86_64
xorriso   (package: libisoburn)
```

On this box, pacman rust stays for the suite. Kernel builds with `~/.cargo/bin/cargo`.

```bash
cd ~/fae-kernel
make            # kernel ELF + ISO (needs xorriso + limine clone)
make qemu       # BIOS QEMU, serial on stdio (needs qemu)
make serial     # same, no window; CI greps fae-kernel
```

`limine/` is cloned at build time (`v10.x-binary`), not vendored.

## Layout

| Path | Role |
|---|---|
| `kernel/` | `no_std` crate |
| `limine.conf` | boot menu |
| `docs/syscalls.md` | living Linux ABI table (honest stubs) |
| `docs/BOOT.md` | Limine vs UEFI, QEMU flags |

## Law

- We write the kernel. We do not copy Linux source. We do not ship an Arch ISO and call it ours.
- A missing syscall returns `ENOSYS`. Silent success is a lie.
- Offline. No telemetry.
- Not inside `~/faeOS` kit git.

## Phases

0 this tree (serial hello) → 1 memory → 2 user ELF `write`/`exit` → 3 fork/exec → … suite as guest.
