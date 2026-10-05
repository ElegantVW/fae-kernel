#!/usr/bin/env python3
"""Cast an 8×16 ASCII font for cerne-fw plane 2 (logo.txt + printable).

Each glyph is an 8×8 bitmap doubled vertically. Bit 7 is the left pixel.
Output: fw/font8x16.bin (128 glyphs × 16 rows = 2048 bytes).
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "fw" / "font8x16.bin"

# 8×8, hex rows, chars 0x20–0x7E. Missing → blank.
# Kindling mark uses space, '*', '/', '\\', '|'.
GLYPHS = {
    0x20: "0000000000000000",
    0x21: "3030303030003000",
    0x22: "6C6C6C0000000000",
    0x23: "6C6CFE6C6CFE6C00",
    0x24: "187E407C027C1800",
    0x25: "C6CC183066C60000",
    0x26: "38286C6C6C10EE00",
    0x27: "3030200000000000",
    0x28: "1830606060301800",
    0x29: "30180C0C0C183000",
    0x2A: "1054387C38541000",  # *
    0x2B: "0010107C10100000",
    0x2C: "0000000000302010",
    0x2D: "0000007C00000000",
    0x2E: "0000000000303000",
    0x2F: "0204081020408000",  # /
    0x30: "384CC6C6C64C3800",
    0x31: "1030101010107C00",
    0x32: "7CC6023C80C6FE00",
    0x33: "7C06063C06067C00",
    0x34: "0C1C2C4CFE0C0C00",
    0x35: "FEC0FC0606C67C00",
    0x36: "3C60C0FCC6C67C00",
    0x37: "FE060C1830303000",
    0x38: "7CC6C67CC6C67C00",
    0x39: "7CC6C67E060C7800",
    0x3A: "0030300000303000",
    0x3B: "0030300000302010",
    0x3C: "0C18306030180C00",
    0x3D: "00007C007C000000",
    0x3E: "6030180C18306000",
    0x3F: "7CC6061C10001000",
    0x40: "7CC6DEDED0C07C00",
    0x41: "386CC6C6FEC6C600",
    0x42: "FCC6C6FCC6C6FC00",
    0x43: "3C66C0C0C0663C00",
    0x44: "F8CCC6C6C6CCF800",
    0x45: "FEC0C0FCC0C0FE00",
    0x46: "FEC0C0FCC0C0C000",
    0x47: "3C66C0CEC6663C00",
    0x48: "C6C6C6FEC6C6C600",
    0x49: "7C10101010107C00",
    0x4A: "3E0C0C0C0CCC7800",
    0x4B: "C6CCD8F0D8CCC600",
    0x4C: "C0C0C0C0C0C0FE00",
    0x4D: "C6EEFED6C6C6C600",
    0x4E: "C6E6F6DECEC6C600",
    0x4F: "7CC6C6C6C6C67C00",
    0x50: "FCC6C6FCC0C0C000",
    0x51: "7CC6C6C6C6CC7600",
    0x52: "FCC6C6FCD8CCC600",
    0x53: "7CC6C07C06C67C00",
    0x54: "FE10101010101000",
    0x55: "C6C6C6C6C6C67C00",
    0x56: "C6C6C66C6C381000",
    0x57: "C6C6C6D6FEEEC600",
    0x58: "C66C3838386CC600",
    0x59: "C66C381010101000",
    0x5A: "FE0C183060C0FE00",
    0x5B: "7830303030307800",
    0x5C: "8040201008040200",  # \
    0x5D: "1E0C0C0C0C0C1E00",
    0x5E: "10386CC600000000",
    0x5F: "00000000000000FF",
    0x60: "3010080000000000",
    0x61: "00007C067EC67E00",
    0x62: "C0C0FCC6C6C6FC00",
    0x63: "00007CC6C0C67C00",
    0x64: "06067EC6C6C67E00",
    0x65: "00007CC6FEC07C00",
    0x66: "1C30307C30303000",
    0x67: "00007EC6C67E067C",
    0x68: "C0C0FCC6C6C6C600",
    0x69: "3000703030307C00",
    0x6A: "0C000C0C0C0C0C78",
    0x6B: "C0C0C6CCD8F0CC00",
    0x6C: "7030303030307C00",
    0x6D: "0000ECFED6C6C600",
    0x6E: "0000FCC6C6C6C600",
    0x6F: "00007CC6C6C67C00",
    0x70: "0000FCC6C6FCC0C0",
    0x71: "00007EC6C67E0606",
    0x72: "0000DCE6C0C0C000",
    0x73: "00007CC07C06FC00",
    0x74: "00307C3030301C00",
    0x75: "0000C6C6C6C67E00",
    0x76: "0000C6C66C381000",
    0x77: "0000C6C6D67C6C00",
    0x78: "0000C66C386CC600",
    0x79: "0000C6C66C3810E0",
    0x7A: "0000FE0C3860FE00",
    0x7B: "1C3030E030301C00",
    0x7C: "1010101010101000",  # |
    0x7D: "E030301C3030E000",
    0x7E: "76DC000000000000",
}


def glyph16(code: int) -> bytes:
    raw = GLYPHS.get(code, "0" * 16)
    rows = bytes.fromhex(raw)
    if len(rows) != 8:
        raise SystemExit(f"glyph {code:#x} is {len(rows)} rows, want 8")
    out = bytearray()
    for b in rows:
        out.append(b)
        out.append(b)
    return bytes(out)


def main() -> None:
    blob = b"".join(glyph16(c) for c in range(128))
    if len(blob) != 2048:
        raise SystemExit(f"font size {len(blob)}, want 2048")
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(blob)
    print(f"mkfont: {OUT.relative_to(ROOT)} ({len(blob)} B)")


if __name__ == "__main__":
    main()
