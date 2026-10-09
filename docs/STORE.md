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

`kindlog` means the first four bytes of that slice are `KLOG`. G45 lays the
superblock and inks the book there.

## Two jobs for FAT

UEFI loads `EFI/BOOT/BOOTX64.EFI` from a FAT ESP. That is why FAT is in the
path at all. Packed **cairn** stays a file on that ESP. `LEAF` and sparks still glean the
FAT root. Live seals (`hands` / `twin` / `hand`) live on KINDLOG at fixed
LBAs: super `KLOG` at relative 0, `hands` at 1–2, `twin` at 3–4, `hand` at 5.
On the Databar that slice is MBR type `0x6c`, LBA 64, 1984 sectors — the
1 MiB headroom before the ESP. FAT create is no longer the account plan.

## Buses

| Bus | Today |
|---|---|
| USB MSC | lit — the Databar, QEMU `usb-storage` |
| AHCI / SATA | dark |
| NVMe | dark |

The IdeaPad's internal disk is invisible until a bus other than MSC lights.
A dump on that laptop is the Databar until then.

## This sitting

KINDLOG is the house. Host plants the MBR slot and superblock. `enlist`
WRITE(10)+rereads only those data LBAs, and only when the FAT arm is
KINDLING `85C7-AA81`. The superblock is not rewritten by the kernel.
