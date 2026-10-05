#!/usr/bin/env python3
"""Cast a Kindling boot disk.

    LBA 0        KMAP   (magic, checksum, kernel_lba/sectors/xor + cairn_*,
                         docs/CAIRN.md - the loader reads only the kernel half)
    LBA 1..32    loader slot (16KiB; trailer 'LDOK' at the end proves the
                 whole slot arrived - the firmware checks it before jumping)
    LBA 33..     kernel (sector-padded)
    ...          cairn (sector-padded leaves + sparks; absent unless asked)

The firmware loads the slot to 0x9000; the loader reads the KMAP, loads the
kernel to 0x200000 and checks both its XOR and 'KNDL' before jumping -
then lays the cairn at 0x100000 the same way.

  --bad     corrupt the KMAP checksum - gate G14: must refuse to boot
  --wrong   a consistent KMAP pointing at empty disk - gate G2 "wrong":
            the 'KNDL' check is the one that catches that
  --leaf name=path    lay a leaf (data) in the cairn (repeatable, name <=64,
                      no '/' - flat grove, house voice)
  --spark name=path   lay a spark (CPL3 flat binary, entry at offset 0)
  --cairn-out path    also write the cairn blob (ESP / EFI/BOOT/CAIRN)
"""
import argparse
import struct
import sys

SECTOR = 512
LD_SLOTS = 32                       # sectors reserved for the loader
LD_LBA = 1
KERNEL_LBA = 1 + LD_SLOTS           # 33
KMAP_MAGIC = 0x50414D4B             # 'KMAP'
CAIR_MAGIC = 0x52494143             # 'CAIR'
CAIR_VERSION = 1
LD_MARKER = b"LDOK"
WRONG_LBA = 2000
WRONG_SECTORS = 4
MAX_LEAF = 1 << 20                  # datalen cap, same honesty as the calls


def xor32(buf: bytes) -> int:
    """XOR of every little-endian dword (zero-padded tail) - the loader does
    the same over the sectors it loads."""
    padded = buf + b"\0" * (-len(buf) % 4)
    value = 0
    for i in range(0, len(padded), 4):
        value ^= struct.unpack_from("<I", padded, i)[0]
    return value


def sector_pad(buf: bytes) -> bytes:
    return buf + b"\0" * (-len(buf) % SECTOR)


def kmap(kernel_lba: int, kernel_sectors: int, kernel_xor: int,
         cairn_lba: int, cairn_sectors: int, cairn_xor: int, bad: bool) -> bytes:
    checksum = kernel_lba ^ kernel_sectors ^ kernel_xor ^ KMAP_MAGIC
    if bad:
        checksum ^= 1
    # Offsets 0-16 never move: the loader's trust is untouched. Cairn fields
    # ride after with their own checksum the kernel checks.
    cairn_sum = cairn_lba ^ cairn_sectors ^ cairn_xor ^ CAIR_MAGIC
    head = struct.pack(
        "<8I", KMAP_MAGIC, checksum, kernel_lba, kernel_sectors, kernel_xor,
        cairn_lba, cairn_sectors, cairn_sum,
    )
    return sector_pad(head)


def cast_leaf(name: str, data: bytes, kind: int) -> bytes:
    if not (1 <= len(name) <= 64) or "/" in name:
        sys.exit(f"mkimg: bad leaf name {name!r} (1-64 chars, no '/')")
    try:
        tag = name.encode("ascii")
    except UnicodeEncodeError:
        sys.exit(f"mkimg: leaf name {name!r} is not ascii")
    if len(data) > MAX_LEAF:
        sys.exit(f"mkimg: leaf {name!r} too big ({len(data)} B)")
    rec = struct.pack("<I", kind) + bytes([len(tag)]) + tag
    rec += b"\0" * (-len(rec) % 4)
    rec += struct.pack("<I", len(data)) + data
    rec += b"\0" * (-len(rec) % 4)
    rec += struct.pack("<I", xor32(data))
    return rec


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--loader", default="ld/cerne-ld.bin")
    ap.add_argument("--kernel", default="kernel/kernel.fw.bin")
    ap.add_argument("--out", default="kindling.img")
    ap.add_argument("--bad", action="store_true", help="corrupt KMAP checksum")
    ap.add_argument("--wrong", action="store_true", help="KMAP points at empty disk")
    ap.add_argument("--leaf", action="append", default=[],
                    help="lay a leaf: name=path (repeatable)")
    ap.add_argument("--spark", action="append", default=[],
                    help="lay a spark: name=path (repeatable)")
    ap.add_argument("--cairn-out", default=None,
                    help="write the cairn blob (needs --leaf/--spark)")
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

    cairn = b""
    if args.leaf or args.spark:
        if args.wrong or args.bad:
            sys.exit("mkimg: --leaf/--spark refuse bad company (--bad/--wrong)")
        recs = b""
        for spec in args.leaf:
            name, _, path = spec.partition("=")
            try:
                recs += cast_leaf(name, open(path, "rb").read(), 0)
            except OSError as exc:
                sys.exit(f"mkimg: {exc}")
        for spec in args.spark:
            name, _, path = spec.partition("=")
            try:
                recs += cast_leaf(name, open(path, "rb").read(), 1)
            except OSError as exc:
                sys.exit(f"mkimg: {exc}")
        nleaves = len(args.leaf) + len(args.spark)
        cairn = struct.pack("<3I", CAIR_MAGIC, CAIR_VERSION, nleaves) + recs
        cairn = sector_pad(cairn)
        cairn_lba = KERNEL_LBA + kernel_sectors
        cairn_sectors = len(cairn) // SECTOR
        cairn_xor = xor32(cairn)
    else:
        cairn_lba, cairn_sectors, cairn_xor = 0, 0, 0

    image = kmap(kernel_lba, kernel_sectors, kernel_xor,
                 cairn_lba, cairn_sectors, cairn_xor, args.bad)
    image += bytes(slot) + body + cairn

    with open(args.out, "wb") as fh:
        fh.write(image)

    if args.cairn_out:
        if not cairn:
            sys.exit("mkimg: --cairn-out needs --leaf or --spark")
        try:
            with open(args.cairn_out, "wb") as fh:
                fh.write(cairn)
        except OSError as exc:
            sys.exit(f"mkimg: {exc}")
        print(f"mkimg: {args.cairn_out} ({cairn_sectors} sectors)")

    note = "bad checksum" if args.bad else ("wrong lba" if args.wrong else "kmap ok")
    if cairn:
        note += f"; cairn {cairn_sectors} sectors at LBA {cairn_lba}"
    print(
        f"mkimg: {args.out} ({note}; loader {len(loader)} B in {LD_SLOTS} sectors, "
        f"kernel {kernel_sectors} sectors at LBA {kernel_lba})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
