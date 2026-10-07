#!/usr/bin/env python3
"""EFI keeper: the book. Create, hall, choose, split.

Happy BOOTX64. Cairn has only ingle. FAT32 at LBA 2048 is KINDLING
`85C7-AA81` with LFN `keeper` and no wax book leaves. First boot creates
`hands`/`twin`/`hand` (716/716/32) and gil with word tinder. Hall answers:
the name is cut, the word sleeps in the twin. Boot2 is the hall: n oak,
choose 2 (this fire knows you). Dismiss gives the name back. A flipped
twin byte prints `the book is split`. A second stick with wax 716/32 still
enlists. Ciphertexts differ. Does not touch the live Databar.
"""
from __future__ import annotations

import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QEMU = os.environ.get("QEMU", "qemu-system-x86_64")
ESP = ROOT / "esp"
EFI = ESP / "EFI" / "BOOT" / "BOOTX64.EFI"
CAIRN = ESP / "EFI" / "BOOT" / "CAIRN"
MON = f"/tmp/kindling-efi-keeper-{os.getpid()}.mon"
SER = f"/tmp/kindling-efi-keeper-{os.getpid()}.ser"
VARS = f"/tmp/kindling-efi-keeper-{os.getpid()}.vars"
ERR = f"/tmp/kindling-efi-keeper-{os.getpid()}.err"
IMG = f"/tmp/kindling-efi-keeper-{os.getpid()}.img"

OVMF_CODE = [
    Path("/usr/share/edk2/x64/OVMF_CODE.4m.fd"),
    Path("/usr/share/edk2/x64/OVMF_CODE.fd"),
    Path("/usr/share/edk2-ovmf/x64/OVMF_CODE.fd"),
    Path("/usr/share/OVMF/OVMF_CODE.fd"),
]
OVMF_VARS = [
    Path("/usr/share/edk2/x64/OVMF_VARS.4m.fd"),
    Path("/usr/share/edk2/x64/OVMF_VARS.fd"),
    Path("/usr/share/edk2-ovmf/x64/OVMF_VARS.fd"),
    Path("/usr/share/OVMF/OVMF_VARS.fd"),
]

NAME = "gil"
NAME2 = "oak"
WORD = "tinder"
CUT = "the name is cut"
SLEEP = "the word sleeps in the twin"
KNOWS = "this fire knows you"
GIVEN = "that name is given back"
HAND_LEN = 32
SEAL_LEN = 716
KINDLING_VOL = 0x85C7AA81
PART_LBA = 2048
CPU = "qemu64,+aes,+rdrand"
LOCS: dict[str, int] = {}


def first_file(cands: list[Path]) -> Path | None:
    for p in cands:
        if p.is_file():
            return p
    return None


def short_cksum(name11: bytes) -> int:
    s = 0
    for b in name11:
        s = (((s & 1) << 7) + (s >> 1) + b) & 0xFF
    return s


def make_83(name: str) -> bytes:
    base = name.upper()[:8]
    return (base.ljust(8) + "   ").encode("ascii")


def lfn_entry(long_name: str, name11: bytes) -> bytes:
    chars = [ord(c) for c in long_name] + [0]
    while len(chars) < 13:
        chars.append(0xFFFF)
    ent = bytearray(32)
    ent[0] = 0x41
    ck = short_cksum(name11)

    def put(start: int, chunk: list[int]) -> None:
        i = 0
        for ch in chunk:
            ent[start + i] = ch & 0xFF
            ent[start + i + 1] = (ch >> 8) & 0xFF
            i += 2

    put(1, chars[0:5])
    ent[11] = 0x0F
    ent[13] = ck
    put(14, chars[5:11])
    put(28, chars[11:13])
    return bytes(ent)


def plant_fat32(img: bytearray, files: list[tuple[str, bytes]], part_lba: int = PART_LBA, part_secs: int = 2048) -> None:
    fat = part_lba * 512
    img[fat : fat + 3] = b"\xeb\x58\x90"
    img[fat + 3 : fat + 11] = b"MSDOS5.0"
    img[fat + 11 : fat + 13] = (512).to_bytes(2, "little")
    img[fat + 13] = 1
    img[fat + 14 : fat + 16] = (32).to_bytes(2, "little")
    img[fat + 16] = 2
    img[fat + 17 : fat + 19] = (0).to_bytes(2, "little")
    img[fat + 19 : fat + 21] = (0).to_bytes(2, "little")
    img[fat + 21] = 0xF8
    img[fat + 22 : fat + 24] = (0).to_bytes(2, "little")
    img[fat + 24 : fat + 26] = (32).to_bytes(2, "little")
    img[fat + 26 : fat + 28] = (2).to_bytes(2, "little")
    img[fat + 28 : fat + 32] = part_lba.to_bytes(4, "little")
    img[fat + 32 : fat + 36] = part_secs.to_bytes(4, "little")
    img[fat + 36 : fat + 40] = (16).to_bytes(4, "little")
    img[fat + 44 : fat + 48] = (2).to_bytes(4, "little")
    img[fat + 48 : fat + 50] = (1).to_bytes(2, "little")
    img[fat + 50 : fat + 52] = (6).to_bytes(2, "little")
    img[fat + 66] = 0x29
    img[fat + 67 : fat + 71] = KINDLING_VOL.to_bytes(4, "little")
    img[fat + 71 : fat + 82] = b"KINDLING   "
    img[fat + 82 : fat + 90] = b"FAT32   "
    img[fat + 510] = 0x55
    img[fat + 511] = 0xAA
    fat1 = fat + 32 * 512
    data = fat + 64 * 512
    entries = bytearray()
    fatmap = {0: 0x0FFFFFF8, 1: 0x0FFFFFFF, 2: 0x0FFFFFFF}
    clus = 3
    LOCS.clear()
    for lfn, blob in files:
        nclus = max(1, (len(blob) + 511) // 512)
        first = clus
        LOCS[lfn] = data + (first - 2) * 512
        for i in range(nclus):
            nxt = 0x0FFFFFFF if i == nclus - 1 else clus + 1
            fatmap[clus] = nxt
            start = data + (clus - 2) * 512
            chunk = blob[i * 512 : (i + 1) * 512]
            img[start : start + len(chunk)] = chunk
            clus += 1
        name11 = make_83(lfn)
        entries += lfn_entry(lfn, name11)
        ent = bytearray(32)
        ent[0:11] = name11
        ent[11] = 0x20
        ent[26:28] = (first & 0xFFFF).to_bytes(2, "little")
        ent[20:22] = (first >> 16).to_bytes(2, "little")
        ent[28:32] = len(blob).to_bytes(4, "little")
        entries += bytes(ent)
    img[data : data + len(entries)] = entries
    for c, val in fatmap.items():
        off = fat1 + c * 4
        img[off : off + 4] = (val & 0x0FFFFFFF).to_bytes(4, "little")
    fat2 = fat + (32 + 16) * 512
    img[fat2 : fat2 + 16 * 512] = img[fat1 : fat1 + 16 * 512]


def write_stick(spark: bytes, wax_book: bool) -> None:
    img = bytearray(2 * 1024 * 1024)
    img[510] = 0x55
    img[511] = 0xAA
    img[446 + 4] = 0x0C
    img[446 + 8 : 446 + 12] = PART_LBA.to_bytes(4, "little")
    img[446 + 12 : 446 + 16] = (2048).to_bytes(4, "little")
    files: list[tuple[str, bytes]] = [("keeper", spark)]
    if wax_book:
        files = [
            ("hand", bytes(HAND_LEN)),
            ("keeper", spark),
            ("hands", bytes(SEAL_LEN)),
            ("twin", bytes(SEAL_LEN)),
        ]
    plant_fat32(img, files)
    fat = PART_LBA * 512
    if int.from_bytes(img[fat + 67 : fat + 71], "little") != KINDLING_VOL:
        raise SystemExit("FAIL efi-keeper (planted volume serial is not KINDLING)")
    Path(IMG).write_bytes(img)


def fat32_meta(img: bytes, part_lba: int = PART_LBA) -> tuple[int, int, int, int]:
    fat = part_lba * 512
    reserved = int.from_bytes(img[fat + 14 : fat + 16], "little")
    fats = img[fat + 16]
    fat_sz = int.from_bytes(img[fat + 36 : fat + 40], "little")
    spc = img[fat + 13]
    root_clus = int.from_bytes(img[fat + 44 : fat + 48], "little")
    fat1 = fat + reserved * 512
    data = fat1 + fats * fat_sz * 512
    return fat1, data, spc, root_clus


def fat_next32(img: bytes, fat1: int, clus: int) -> int:
    off = fat1 + clus * 4
    v = int.from_bytes(img[off : off + 4], "little") & 0x0FFFFFFF
    if v >= 0x0FFFFFF8:
        return 0
    return v


def walk_root(img: bytes, part_lba: int = PART_LBA) -> dict[str, tuple[int, int]]:
    """LFN (or 8.3-only) name → (data offset in the image, size)."""
    fat1, data, spc, root_clus = fat32_meta(img, part_lba)
    bpc = spc * 512
    files: dict[str, tuple[int, int]] = {}
    lfn_map: dict[int, list[int]] = {}
    clus = root_clus
    seen = 0
    while clus >= 2 and seen < 256:
        base = data + (clus - 2) * bpc
        for e in range(0, bpc, 32):
            ent = img[base + e : base + e + 32]
            if len(ent) < 32:
                return files
            first = ent[0]
            if first == 0:
                return files
            if first == 0xE5:
                lfn_map.clear()
                continue
            attr = ent[11]
            if attr == 0x0F:
                seq = first & 0x3F
                chars: list[int] = []
                for start, n in ((1, 5), (14, 6), (28, 2)):
                    for i in range(n):
                        chars.append(ent[start + 2 * i] | (ent[start + 2 * i + 1] << 8))
                lfn_map[seq] = chars
                continue
            if attr & 0x18:
                lfn_map.clear()
                continue
            name = ""
            if lfn_map:
                chars = []
                i = 1
                while i in lfn_map:
                    chars.extend(lfn_map[i])
                    i += 1
                s: list[str] = []
                for cp in chars:
                    if cp == 0:
                        break
                    if 32 <= cp < 127:
                        s.append(chr(cp))
                name = "".join(s)
                lfn_map.clear()
            else:
                base_n = ent[0:8].decode("ascii", "replace").rstrip()
                ext = ent[8:11].decode("ascii", "replace").rstrip()
                name = f"{base_n}.{ext}" if ext else base_n
            hi = int.from_bytes(ent[20:22], "little")
            lo = int.from_bytes(ent[26:28], "little")
            size = int.from_bytes(ent[28:32], "little")
            first_clus = (hi << 16) | lo
            off = data + (first_clus - 2) * bpc if first_clus >= 2 else 0
            if name:
                files[name] = (off, size)
        nxt = fat_next32(img, fat1, clus)
        if nxt == 0:
            break
        clus = nxt
        seen += 1
    return files


def remember_book(img: bytes) -> str | None:
    files = walk_root(img)
    for nm, sz in (("hands", SEAL_LEN), ("twin", SEAL_LEN), ("hand", HAND_LEN)):
        if nm not in files:
            return f"create never laid {nm}"
        off, size = files[nm]
        if size != sz:
            return f"{nm} is {size}, want {sz}"
        LOCS[nm] = off
    hoff = LOCS["hand"]
    if img[hoff : hoff + len(NAME)] != NAME.encode() or img[hoff + len(NAME)] != 0:
        return "WRITE(10) never landed the name"
    hands = img[LOCS["hands"] : LOCS["hands"] + SEAL_LEN]
    twin = img[LOCS["twin"] : LOCS["twin"] + SEAL_LEN]
    if hands == twin:
        return "hands and twin ciphertexts compare equal"
    if all(b == 0 for b in hands) or all(b == 0 for b in twin):
        return "book still wax after enlist"
    if WORD.encode() in img:
        return "the word is on the disk"
    if NAME.encode() in hands and NAME.encode() in twin:
        return "keeper names sit in the sealed blobs"
    return None


def read_until(sock: socket.socket, mark: str, timeout: float) -> str:
    buf = ""
    end = time.time() + timeout
    sock.settimeout(0.2)
    while time.time() < end:
        try:
            chunk = sock.recv(4096)
        except TimeoutError:
            chunk = b""
        except socket.timeout:
            chunk = b""
        if chunk:
            buf += chunk.decode(errors="replace")
        if mark in buf:
            return buf
    return buf


def serial_has(mark: str, timeout: float) -> str:
    end = time.time() + timeout
    text = ""
    while time.time() < end:
        if Path(SER).is_file():
            text = Path(SER).read_bytes().decode(errors="replace").replace("\r", "")
            if mark in text:
                return text
        time.sleep(0.05)
    return text


def send_keys(sock: socket.socket, keys: str) -> None:
    for ch in keys:
        if ch == "\n":
            sock.sendall(b"sendkey ret\n")
        elif ch == "\b":
            sock.sendall(b"sendkey backspace\n")
        else:
            sock.sendall(f"sendkey {ch}\n".encode())
        read_until(sock, "(qemu)", 2)


def crush(s: str) -> str:
    out: list[str] = []
    for c in s:
        if c == "\b" or c == "\x08":
            if out:
                out.pop()
        else:
            out.append(c)
    return "".join(out)


def boot_once(code: Path) -> subprocess.Popen:
    try:
        os.unlink(SER)
    except FileNotFoundError:
        pass
    try:
        os.unlink(MON)
    except FileNotFoundError:
        pass
    errf = open(ERR, "ab")
    return subprocess.Popen(
        [
            QEMU,
            "-M",
            "q35",
            "-cpu",
            CPU,
            "-m",
            "256M",
            "-display",
            "none",
            "-serial",
            f"file:{SER}",
            "-no-reboot",
            "-no-shutdown",
            "-drive",
            f"if=pflash,format=raw,unit=0,readonly=on,file={code}",
            "-drive",
            f"if=pflash,format=raw,unit=1,file={VARS}",
            "-drive",
            "format=raw,file=fat:rw:esp",
            "-drive",
            f"if=none,id=kindstick,format=raw,cache=writethrough,file={IMG}",
            "-device",
            "qemu-xhci,id=xhci",
            "-device",
            "usb-kbd,bus=xhci.0,port=1",
            "-device",
            "usb-storage,bus=xhci.0,port=2,drive=kindstick",
            "-monitor",
            f"unix:{MON},server,nowait",
        ],
        stdout=subprocess.DEVNULL,
        stderr=errf,
        cwd=ROOT,
        start_new_session=True,
    )


def stop(proc: subprocess.Popen) -> None:
    try:
        os.killpg(proc.pid, 9)
    except OSError:
        proc.kill()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.send_signal(9)


def monitor() -> socket.socket | None:
    deadline = time.time() + 2
    while time.time() < deadline and not Path(MON).exists():
        time.sleep(0.05)
    if not Path(MON).exists():
        return None
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.settimeout(2)
    sock.connect(MON)
    banner = read_until(sock, "(qemu)", 2)
    if "(qemu)" not in banner:
        sock.close()
        return None
    return sock


def fail(msg: str, serial: str = "") -> int:
    print(f"FAIL efi-keeper ({msg})")
    if serial:
        print(serial[-600:])
    return 1


def wait_greet(after: str, name: str, timeout: float) -> str:
    end = time.time() + timeout
    text = ""
    while time.time() < end:
        if Path(SER).is_file():
            text = Path(SER).read_bytes().decode(errors="replace").replace("\r", "")
            mark = text.rfind(after)
            tail = crush(text[mark:]) if mark >= 0 else ""
            if f"\n{name}\n" in tail or tail.endswith(f"\n{name}") or tail.endswith(f"{name}\n"):
                return text
        time.sleep(0.05)
    return text


def proof_wax(code: Path, spark: bytes) -> int:
    write_stick(spark, wax_book=True)
    proc = boot_once(code)
    sock = None
    try:
        serial = serial_has("who keeps this fire", 25)
        t = serial.replace("\r", "")
        if "who keeps this fire" not in t:
            return fail("wax: no keeper prompt", serial)
        sock = monitor()
        if sock is None:
            return fail("wax: no monitor")
        send_keys(sock, f"{NAME}\n")
        serial = serial_has("speak the word", 8)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has("speak it again", 8)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has(CUT, 8)
        t = serial.replace("\r", "")
        if CUT not in t:
            return fail("wax: enlist did not cut the name", t)
        serial = serial_has(SLEEP, 8)
        t = serial.replace("\r", "")
        if SLEEP not in t:
            return fail("wax: enlist did not lay the word in the twin", t)
    finally:
        if sock is not None:
            sock.close()
        stop(proc)
    miss = remember_book(Path(IMG).read_bytes())
    if miss:
        return fail("wax: " + miss)
    return 0


def enter_and_leave(sock: socket.socket) -> str | None:
    send_keys(sock, "\n")
    serial = serial_has("the fire is lit", 8)
    t = serial.replace("\r", "")
    if "the fire is lit" not in t:
        return "no fire"
    send_keys(sock, "q")
    serial = serial_has("kindling: gleam exit 0", 8)
    t = serial.replace("\r", "")
    if "kindling: gleam exit 0" not in t:
        return "no gleam exit 0"
    if WORD in t:
        return "the word leaked on serial"
    return None


def main() -> int:
    for p in (MON, SER, VARS, ERR, IMG):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    code = first_file(OVMF_CODE)
    vars_src = first_file(OVMF_VARS)
    if code is None or vars_src is None:
        print("FAIL efi-keeper (no OVMF)", file=sys.stderr)
        return 1
    src = ROOT / "kernel" / "BOOTX64.EFI"
    ld = ROOT / "ld" / "cerne-ld.bin"
    kern = ROOT / "kernel" / "kernel.fw.bin"
    if not src.is_file():
        print("FAIL efi-keeper (missing BOOTX64.EFI)", file=sys.stderr)
        return 1
    if not ld.is_file() or not kern.is_file():
        print("FAIL efi-keeper (missing loader or kernel)", file=sys.stderr)
        return 1
    subprocess.check_call(
        ["nasm", "-f", "bin", "-o", str(ROOT / "spark" / "ingle.bin"), str(ROOT / "spark" / "ingle.asm")],
        cwd=ROOT,
    )
    subprocess.check_call(
        ["nasm", "-f", "bin", "-o", str(ROOT / "spark" / "keeper.bin"), str(ROOT / "spark" / "keeper.asm")],
        cwd=ROOT,
    )
    spark = (ROOT / "spark" / "keeper.bin").read_bytes()
    if not spark or len(spark) > 8192:
        print("FAIL efi-keeper (keeper spark empty or too big)", file=sys.stderr)
        return 1
    cairn_blob = ROOT / "spark" / "cairn-ingle.bin"
    subprocess.check_call(
        [
            "python3",
            str(ROOT / "scripts" / "mkimg.py"),
            "--loader",
            str(ld),
            "--kernel",
            str(kern),
            "--out",
            str(ROOT / "kindling-efi-keeper-pack.img"),
            "--spark",
            f"ingle={ROOT / 'spark' / 'ingle.bin'}",
            "--cairn-out",
            str(cairn_blob),
        ],
        cwd=ROOT,
    )
    ESP.joinpath("EFI/BOOT").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(src, EFI)
    shutil.copyfile(cairn_blob, CAIRN)
    write_stick(spark, wax_book=False)
    planted = walk_root(Path(IMG).read_bytes())
    if "hands" in planted or "twin" in planted or "hand" in planted:
        print("FAIL efi-keeper (wax book files present on create proof)", file=sys.stderr)
        return 1
    shutil.copyfile(vars_src, VARS)

    proc = boot_once(code)
    sock = None
    try:
        serial = serial_has("kindling: ingle ok\ningle\n", 25)
        t = serial.replace("\r", "")
        if "kindling: fat" not in t:
            return fail("no fat", serial)
        if "kindling: ingle ok\ningle\n" not in t:
            return fail("spark never wrote its name", serial)
        serial = serial_has("who keeps this fire", 8)
        t = serial.replace("\r", "")
        if "who keeps this fire" not in t:
            return fail("no keeper prompt on first boot", serial)
        sock = monitor()
        if sock is None:
            return fail("no monitor")
        send_keys(sock, "gix\bl\n")
        serial = serial_has("speak the word", 8)
        t = serial.replace("\r", "")
        mark = t.find("who keeps this fire")
        if mark < 0:
            return fail("prompt vanished", serial)
        echoed = crush(t[mark:])
        if "gix" not in t:
            return fail("name keys never echoed", serial)
        if NAME not in echoed:
            return fail("echo+backspace did not land gil", serial)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has("speak it again", 8)
        t = serial.replace("\r", "")
        if "speak it again" not in t:
            return fail("no confirm prompt", serial)
        after_word = t[t.find("speak the word") :]
        if WORD in after_word:
            return fail("the word leaked on serial", serial)
        if "*" not in after_word:
            return fail("word did not echo as stars", serial)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has(CUT, 15)
        t = serial.replace("\r", "")
        if CUT not in t:
            return fail("enlist did not cut the name", t)
        serial = serial_has(SLEEP, 8)
        t = serial.replace("\r", "")
        if SLEEP not in t:
            return fail("enlist did not lay the word in the twin", t)
        t = wait_greet(SLEEP, NAME, 8)
        tail = crush(t[t.rfind(SLEEP) :]) if SLEEP in t else t
        if f"\n{NAME}\n" not in tail and not tail.endswith(f"{NAME}\n") and not tail.endswith(f"\n{NAME}"):
            return fail("ingle did not greet after enlist", t)
        err = enter_and_leave(sock)
        if err:
            return fail(err + " on first boot", t)
        sock.close()
        sock = None
    finally:
        if sock is not None:
            sock.close()
        stop(proc)

    miss = remember_book(Path(IMG).read_bytes())
    if miss:
        return fail(miss)

    proc = boot_once(code)
    sock = None
    try:
        serial = serial_has("who keeps this fire", 25)
        t = serial.replace("\r", "")
        if "who keeps this fire" not in t:
            return fail("no hall on second boot", serial)
        if "1 gil" not in t:
            return fail("hall did not list gil", serial)
        sock = monitor()
        if sock is None:
            return fail("no monitor on second boot")
        send_keys(sock, "n")
        serial = serial_has("who keeps this fire", 8)
        t = serial.replace("\r", "")
        send_keys(sock, f"{NAME2}\n")
        serial = serial_has("speak the word", 8)
        if "speak the word" not in serial.replace("\r", ""):
            return fail("no word prompt for new keeper", serial)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has("speak it again", 8)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has(CUT, 8)
        t = serial.replace("\r", "")
        if CUT not in t:
            return fail("new keeper did not cut the name", t)
        serial = serial_has("2 oak", 8)
        t = serial.replace("\r", "")
        if "2 oak" not in t:
            return fail("hall did not list oak after enlist", t)
        send_keys(sock, "2")
        serial = serial_has("speak the word", 8)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has(KNOWS, 8)
        t = serial.replace("\r", "")
        if KNOWS not in t:
            return fail("choose did not answer", t)
        t = wait_greet(KNOWS, NAME2, 8)
        tail = crush(t[t.rfind(KNOWS) :]) if KNOWS in t else t
        if f"\n{NAME2}\n" not in tail and not tail.endswith(f"{NAME2}\n") and not tail.endswith(f"\n{NAME2}"):
            return fail("choose 2 did not greet oak", t)
        err = enter_and_leave(sock)
        if err:
            return fail(err + " after choose oak", t)
        sock.close()
        sock = None
    finally:
        if sock is not None:
            sock.close()
        stop(proc)

    proc = boot_once(code)
    sock = None
    try:
        serial = serial_has("who keeps this fire", 25)
        t = serial.replace("\r", "")
        if "1 gil" not in t or "2 oak" not in t:
            return fail("hall lost a keeper", t)
        sock = monitor()
        if sock is None:
            return fail("no monitor on dismiss boot")
        send_keys(sock, "d")
        serial = serial_has("who keeps this fire", 8)
        send_keys(sock, "1\n")
        serial = serial_has("speak the word", 8)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has(GIVEN, 8)
        t = serial.replace("\r", "")
        if GIVEN not in t:
            return fail("dismiss did not give the name back", t)
        serial = serial_has("who keeps this fire", 8)
        t = serial.replace("\r", "")
        if "the last hand stays" in t:
            return fail("dismissed a keeper that was not last, but last stayed", t)
        send_keys(sock, "d")
        serial = serial_has("who keeps this fire", 8)
        send_keys(sock, "1\n")
        serial = serial_has("speak the word", 8)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has("the last hand stays", 8)
        t = serial.replace("\r", "")
        if "the last hand stays" not in t:
            return fail("last keeper was dismissed", t)
        send_keys(sock, "1")
        serial = serial_has("speak the word", 8)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has(KNOWS, 8)
        t = wait_greet(KNOWS, NAME2, 8)
        tail = crush(t[t.rfind(KNOWS) :]) if KNOWS in t else t
        if f"\n{NAME2}\n" not in tail and not tail.endswith(f"{NAME2}\n") and not tail.endswith(f"\n{NAME2}"):
            return fail("remaining keeper did not greet", t)
        err = enter_and_leave(sock)
        if err:
            return fail(err + " after last-hand proof", t)
        sock.close()
        sock = None
    finally:
        if sock is not None:
            sock.close()
        stop(proc)

    split = bytearray(Path(IMG).read_bytes())
    split[LOCS["twin"] + 20] ^= 0x01
    Path(IMG).write_bytes(split)
    proc = boot_once(code)
    sock = None
    try:
        serial = serial_has("the book is split", 25)
        t = serial.replace("\r", "")
        if "the book is split" not in t:
            return fail("split twin was not caught", serial)
        if "the fire is lit" in t:
            return fail("split book still lit the fire", serial)
    finally:
        if sock is not None:
            sock.close()
        stop(proc)

    wax = proof_wax(code, spark)
    if wax != 0:
        return wax
    print("ok   efi-keeper")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    finally:
        for p in (MON, SER, VARS, ERR, IMG):
            try:
                os.unlink(p)
            except FileNotFoundError:
                pass
        try:
            os.unlink(ROOT / "kindling-efi-keeper-pack.img")
        except FileNotFoundError:
            pass
