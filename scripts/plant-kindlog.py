#!/usr/bin/env python3
"""Plant a KINDLOG slice on a raw disk (the Databar) or an image file.

Does not shrink FAT. Slot 0 must already be the KINDLING ESP at LBA 2048.
Slot 1 must be empty, or already this KINDLOG. Superblock is rewritten.
A sealed book (name in `hand`) stays; gap entropy is waxed so the hall
does not split. Does not touch EFI files.
"""
from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import kindlog  # noqa: E402


def load(dev: str) -> bytearray:
    with open(dev, "rb") as f:
        if os.path.isfile(dev):
            return bytearray(f.read())
        mbr = f.read(512)
        if len(mbr) != 512:
            raise SystemExit("FAIL plant-kindlog (short MBR)")
        # Need through LBA 69 for super + book slots; write only MBR + LBA 64.
        return bytearray(mbr)


def slot(img: bytes, n: int) -> tuple[int, int, int]:
    e = 446 + n * 16
    ty = img[e + 4]
    start = int.from_bytes(img[e + 8 : e + 12], "little")
    secs = int.from_bytes(img[e + 12 : e + 16], "little")
    return ty, start, secs


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dev", required=True, help="raw disk (e.g. /dev/sdb) or image")
    args = ap.parse_args()
    dev = args.dev
    if not os.path.exists(dev):
        print(f"FAIL plant-kindlog (missing {dev})", file=sys.stderr)
        return 1

    with open(dev, "rb") as f:
        mbr = bytearray(f.read(512))
    if len(mbr) != 512 or mbr[510:512] != b"\x55\xaa":
        print("FAIL plant-kindlog (no MBR)", file=sys.stderr)
        return 1
    ty0, start0, secs0 = slot(mbr, 0)
    ty1, start1, secs1 = slot(mbr, 1)
    if ty0 != 0x0C or start0 != 2048:
        print(f"FAIL plant-kindlog (slot0 is 0x{ty0:02x} @ {start0}, want 0x0c @ 2048)", file=sys.stderr)
        return 1
    if ty1 not in (0, kindlog.KLOG_TYPE):
        print(f"FAIL plant-kindlog (slot1 occupied 0x{ty1:02x})", file=sys.stderr)
        return 1
    if ty1 == kindlog.KLOG_TYPE and (start1 != kindlog.KLOG_START or secs1 != kindlog.KLOG_SECS):
        print("FAIL plant-kindlog (slot1 is KINDLOG at another place)", file=sys.stderr)
        return 1
    if start0 < kindlog.KLOG_START + kindlog.KLOG_SECS:
        print("FAIL plant-kindlog (FAT overlaps KINDLOG headroom)", file=sys.stderr)
        return 1

    kindlog.put_mbr_slot(mbr, 1, kindlog.KLOG_TYPE, kindlog.KLOG_START, kindlog.KLOG_SECS)
    super_blob = kindlog.superblock()
    book_off = (kindlog.KLOG_START + 1) * 512
    book_len = 5 * 512
    waxed = False
    with open(dev, "rb+") as f:
        f.write(mbr)
        f.seek(kindlog.KLOG_START * 512)
        f.write(super_blob)
        f.seek(book_off)
        data = f.read(book_len)
        mini = bytearray((kindlog.KLOG_START + 6) * 512)
        mini[kindlog.KLOG_START * 512 : kindlog.KLOG_START * 512 + 512] = super_blob
        if len(data) == book_len:
            mini[book_off : book_off + book_len] = data
        if not kindlog.held(bytes(mini)):
            f.seek(book_off)
            f.write(bytes(book_len))
            waxed = True
        f.flush()
        os.fsync(f.fileno())
        f.seek(0)
        got_mbr = f.read(512)
        f.seek(kindlog.KLOG_START * 512)
        got_super = f.read(512)
        f.seek(book_off)
        got_book = f.read(book_len)
    if got_mbr[446 + 16 + 4] != kindlog.KLOG_TYPE:
        print("FAIL plant-kindlog (MBR slot1 did not stick)", file=sys.stderr)
        return 1
    if got_super[:4] != kindlog.KLOG_MAGIC:
        print("FAIL plant-kindlog (super did not stick)", file=sys.stderr)
        return 1
    if waxed and got_book != bytes(book_len):
        print("FAIL plant-kindlog (book wax did not stick)", file=sys.stderr)
        return 1
    print(
        f"ok   kindlog 0x{kindlog.KLOG_TYPE:02x} {kindlog.KLOG_START} {kindlog.KLOG_SECS} "
        f"slot0 0x{ty0:02x} {start0} {secs0}"
        + (" waxed" if waxed else " held")
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
