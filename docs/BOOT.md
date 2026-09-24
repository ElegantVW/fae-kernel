# Boot

## Limine (what we use)

Firmware (BIOS or UEFI) loads **Limine**. Limine reads `limine.conf`, loads our ELF to the top 2GiB (`0xffffffff80000000`), and jumps to `kmain` in long mode.

We ask Limine for a framebuffer (optional) and we talk to COM1 (`0x3F8`) ourselves. QEMU `-serial stdio` is that port.

ISO recipe: Limine BIOS CD + UEFI CD in one El Torito image (`xorriso`). `make qemu` uses **BIOS** (`-M q35 -cdrom`) so we do not need OVMF for the serial demo.

## Raw UEFI (what we did not pick)

Our kernel would be `BOOTX64.EFI`. We call Boot Services for the memory map and GOP, then `ExitBootServices`, then we own the CPU. No Limine binary. More code in phase 0, closer to “just firmware + us.” Revisit if Limine becomes a problem on real iron.

## QEMU

```bash
make serial
# qemu-system-x86_64 -M q35 -cdrom fae-kernel.iso -serial stdio -display none -no-reboot
```

The serial log must contain the line `fae-kernel`.
