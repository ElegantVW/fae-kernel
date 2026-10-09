# Store — what Kindling can see

After the well, Kindling owns the buses. `store` walks whatever MSC already
opened, names every partition, and leaves unknown kinds unnamed as mounts.
FAT is one arm: the EFI System Partition Kindling booted from. The house
(accounts, live seals) does not grow there.

```
USB MSC (lit)  →  MBR / GPT  →  fat | kindlog | empty | other
AHCI  (dark)
NVMe  (dark)
```

## Glass

After `msc`:

```
store mbr 512 8192
0c 2048 fat
83 4096 other
fat
```

The header is table (`mbr` / `gpt` / `disk` / `none` / `miss`), block size,
LBA count. Each following line is type hex, start LBA, and a word. GPT type
is the first group of the type GUID. `fat` on its own line is the glean arm
claimed from the first FAT slice.

`kindlog` means the first four bytes of that slice are `KLOG`. G44 names it.
G45 lays the superblock.

## Two jobs for FAT

UEFI loads `EFI/BOOT/BOOTX64.EFI` from a FAT ESP. That is why FAT is in the
path at all. Packed **cairn** stays a file on that ESP. Live seals (`hands` /
`twin` / `LEAF` / wax `hand`) were FAT dirents on the same volume; create on
a live ESP failed iron. `glean` / `stow` / `spawn` still speak FAT this
sitting so the hall keeps working. KINDLOG is the house volume.

## Buses

| Bus | Today |
|---|---|
| USB MSC | lit — the Databar, QEMU `usb-storage` |
| AHCI / SATA | dark |
| NVMe | dark |

The IdeaPad's internal disk is invisible until a bus other than MSC lights.
A dump on that laptop is the Databar until then.

## This sitting

Read only. `store::probe` after the USB host is live. No partition of the
Databar. No KINDLOG ink. Iron photo of this glass is the map for G45.
