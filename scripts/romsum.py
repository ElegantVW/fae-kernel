#!/usr/bin/env python3
"""Last byte of a 64KiB BIOS image so the sum of all bytes is 0."""
import sys

path = sys.argv[1]
data = bytearray(open(path, "rb").read())
if len(data) != 65536:
    sys.exit(f"romsum: want 65536, got {len(data)}")
data[-1] = 0
data[-1] = (-sum(data)) & 0xFF
open(path, "wb").write(data)
if sum(data) & 0xFF:
    sys.exit("romsum: checksum failed")
print(f"romsum: ok ({path})")
