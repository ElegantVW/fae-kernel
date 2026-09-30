#!/usr/bin/env python3
"""Cast a Kindling boot disk.

    LBA 0        KMAP   (magic, checksum, kernel_lba/sectors/xor)
    LBA 1..32    loader slot (16KiB; trailer 'LDOK' at the end proves the
                 whole slot arrived — the firmware checks it before jumping)
    LBA 33..     kernel (sector-padded)

The firmware loads the slot to 0x9000; the loader reads the KMAP, loads the
kernel to 0x200000 and checks both its XOR and 'KNDL' before jumping.

  --bad     corrupt the KMAP checksum — gate G14: must refuse to boot
  --wrong   a consistent KMAP pointing at empty disk — gate G2 "wrong":
            the 'KNDL' check is the one that catches it
"""
import argparse
import struct
import sys

SECTOR = 512
LD_SLOTS = 32                       # sectors reserved for the loader
LD_LBA = 1
KERNEL_LBA = 1 + LD_SLOTS           # 33
KMAP_MAGIC = 0x50414D4B             # 'KMAP'
LD_MARKER = b"LDOK"
WRONG_LBA = 2000
WRONG_SECTORS = 4


def xor32(buf: bytes) -> int:
    """XOR of every little-endian dword (zero-padded tail) — the loader does
    the same over the sectors it loads."""
    padded = buf + b"\0" * (-len(buf) % 4)
    value = 0
    for i in range(0, len(padded), 4):
        value ^= struct.unpack_from("<I", padded, i)[0]
    return value


def sector_pad(buf: bytes) -> bytes:
    return buf + b"\0" * (-len(buf) % SECTOR)


def kmap(kernel_lba: int, kernel_sectors: int, kernel_xor: int, bad: bool) -> bytes:
    checksum = kernel_lba ^ kernel_sectors ^ kernel_xor ^ KMAP_MAGIC
    if bad:
        checksum ^= 1
    head = struct.pack(
        "<5I", KMAP_MAGIC, checksum, kernel_lba, kernel_sectors, kernel_xor,
    )
    return sector_pad(head)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--loader", default="ld/cerne-ld.bin")
    ap.add_argument("--kernel", default="kernel/kernel.fw.bin")
    ap.add_argument("--out", default="kindling.img")
    ap.add_argument("--bad", action="store_true", help="corrupt KMAP checksum")
    ap.add_argument("--wrong", action="store_true", help="KMAP points at empty disk")
    args = ap.parse_args()

    try:
        loader = open(args.loader, "rb").read()
        kernel = open(args.kernel, "rb").read()
    except OSError as exc:
        sys.exit(f"mkimg: {exc}")

    slot_size = LD_SLOTS * SECTOR
    if len(loader) + len(LD_MARKER) > slot_size:
        sys.exit(
            f"mkimg: loader {len(loader)} B does not fit the {slot_size} B slot "
            f"(with the {len(LD_MARKER)} B trailer)"
        )

    slot = bytearray(slot_size)
    slot[: len(loader)] = loader
    slot[-len(LD_MARKER):] = LD_MARKER

    if args.wrong:
        kernel_lba, kernel_sectors, kernel_xor = WRONG_LBA, WRONG_SECTORS, 0
        # the file covers the empty area the KMAP points at
        body = b"\0" * (WRONG_LBA + WRONG_SECTORS - KERNEL_LBA) * SECTOR
    else:
        body = sector_pad(kernel)
        kernel_lba = KERNEL_LBA
        kernel_sectors = len(body) // SECTOR
        kernel_xor = xor32(body)

    image = kmap(kernel_lba, kernel_sectors, kernel_xor, args.bad) + bytes(slot) + body

    with open(args.out, "wb") as fh:
        fh.write(image)

    note = "bad checksum" if args.bad else ("wrong lba" if args.wrong else "kmap ok")
    print(
        f"mkimg: {args.out} ({note}; loader {len(loader)} B in {LD_SLOTS} sectors, "
        f"kernel {kernel_sectors} sectors at LBA {kernel_lba})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
