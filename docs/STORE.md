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
0c 2048 2048 fat
83 4096 1024 other
fat
the log is lit
```

The header is table (`mbr` / `gpt` / `disk` / `none` / `miss`), block size,
LBA count. Each following line is type hex, start LBA, sector count, and a
word. GPT type is the first group of the type GUID. `fat` on its own line is
the glean arm claimed from the first FAT slice. `the log is lit` means the
KINDLOG super checked out (glass `the log is dark` if the slice is named
and the super did not).

`kindlog` means the first four bytes of that slice are `KLOG`, or MBR type
`0x6c` after a second read. G45 lays the superblock; G47 caches a live
super at probe (`the log is lit`) and inks the book there. Gap entropy is
waxed so leftover reserved-area bytes do not split the hall.

## Two jobs for FAT

UEFI loads `EFI/BOOT/BOOTX64.EFI` from a FAT ESP. That is why FAT is in the
path at all. Packed **cairn** stays a file on that ESP. `LEAF` and sparks still glean the
FAT root. Live seals (`hands` / `twin` / `hand`) skip the cairn when a
KINDLOG slice is named — the paved pack still carries wax copies. They live
on KINDLOG at fixed
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

KINDLOG is the house. Host plants the MBR slot and superblock, and waxes
the data LBAs unless a sealed book is already held. `enlist` WRITE(10)+rereads
only those data LBAs, and only when the FAT arm is KINDLING `85C7-AA81`.
Book names never fall through to FAT while a KINDLOG slice is named; a
super that does not check out paints `the log is dark`. The superblock is
not rewritten by the kernel. Live is the `KLOG` super (magic, version,
KINDLING vol, sector count, xor).
