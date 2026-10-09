"""KINDLOG slice: MBR type 0x6C, superblock KLOG, book at fixed LBAs."""

from __future__ import annotations

KLOG_MAGIC = b"KLOG"
KLOG_VER = 1
KLOG_START = 64
KLOG_SECS = 1984  # LBA 64 .. 2047, against a FAT ESP at 2048
KLOG_TYPE = 0x6C
KINDLING_VOL = 0x85C7AA81
REL_HANDS = 1
REL_TWIN = 3
REL_HAND = 5
SEAL_LEN = 716
HAND_LEN = 32


def checksum(ver: int, vol: int, secs: int) -> int:
    mag = int.from_bytes(KLOG_MAGIC, "little")
    return (mag ^ ver ^ vol ^ secs) & 0xFFFFFFFF


def superblock(n_secs: int = KLOG_SECS, vol: int = KINDLING_VOL) -> bytes:
    s = bytearray(512)
    s[0:4] = KLOG_MAGIC
    s[4:8] = KLOG_VER.to_bytes(4, "little")
    s[8:12] = vol.to_bytes(4, "little")
    s[12:16] = n_secs.to_bytes(4, "little")
    s[16:20] = checksum(KLOG_VER, vol, n_secs).to_bytes(4, "little")
    return bytes(s)


def put_mbr_slot(img: bytearray, slot: int, ty: int, start: int, secs: int) -> None:
    e = 446 + slot * 16
    img[e + 4] = ty
    img[e + 8 : e + 12] = start.to_bytes(4, "little")
    img[e + 12 : e + 16] = secs.to_bytes(4, "little")


def plant_slice(
    img: bytearray,
    start: int = KLOG_START,
    secs: int = KLOG_SECS,
    vol: int = KINDLING_VOL,
    hands: bytes | None = None,
    twin: bytes | None = None,
    hand: bytes | None = None,
) -> None:
    """MBR slot 1 + superblock. Leaves data sectors alone unless given."""
    if start == 0 or secs < 6:
        raise ValueError("KINDLOG slice too small")
    if start + secs > len(img) // 512:
        raise ValueError("KINDLOG slice past end of image")
    img[510] = 0x55
    img[511] = 0xAA
    put_mbr_slot(img, 1, KLOG_TYPE, start, secs)
    off = start * 512
    img[off : off + 512] = superblock(secs, vol)
    if hands is not None:
        blob = hands + bytes(1024 - len(hands))
        img[off + REL_HANDS * 512 : off + REL_HANDS * 512 + 1024] = blob[:1024]
    if twin is not None:
        blob = twin + bytes(1024 - len(twin))
        img[off + REL_TWIN * 512 : off + REL_TWIN * 512 + 1024] = blob[:1024]
    if hand is not None:
        blob = hand + bytes(512 - len(hand))
        img[off + REL_HAND * 512 : off + REL_HAND * 512 + 512] = blob[:512]


def locs(start: int = KLOG_START) -> dict[str, int]:
    base = start * 512
    return {
        "hands": base + REL_HANDS * 512,
        "twin": base + REL_TWIN * 512,
        "hand": base + REL_HAND * 512,
        "super": base,
    }


def remember_book(img: bytes, name: str, word: str, start: int = KLOG_START) -> str | None:
    loc = locs(start)
    super_off = loc["super"]
    if img[super_off : super_off + 4] != KLOG_MAGIC:
        return "KINDLOG superblock missing"
    hands = img[loc["hands"] : loc["hands"] + SEAL_LEN]
    twin = img[loc["twin"] : loc["twin"] + SEAL_LEN]
    hand = img[loc["hand"] : loc["hand"] + HAND_LEN]
    if hands == twin:
        return "hands and twin ciphertexts compare equal"
    if all(b == 0 for b in hands) or all(b == 0 for b in twin):
        return "book still wax after enlist"
    nb = name.encode()
    if hand[: len(nb)] != nb or hand[len(nb)] != 0:
        return "WRITE(10) never landed the name"
    if word.encode() in img:
        return "the word is on the disk"
    if nb in hands and nb in twin:
        return "keeper names sit in the sealed blobs"
    return None
