#!/usr/bin/env python3
"""Boot EFI Kindling with qemu-xhci + usb-kbd + usb-storage.

Port 1 is the keyboard so a HID-first scan would miss the stick on port 2.
READ CAPACITY must print `kindling: msc`; READ(10) of the FAT partition
boot sector prints `kindling: fat`. The image is MBR + a real FAT32 at
LBA 2048 (like the Databar) with Gleam leaf `LEAF` (the page) and LFN
`leaf` (the spark). Cairn has only ingle, so spawn gathers from the volume.
Enter greets; q homes leaf (`the light remains`); q leaves.
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
MON = f"/tmp/kindling-efi-msc-{os.getpid()}.mon"
SER = f"/tmp/kindling-efi-msc-{os.getpid()}.ser"
VARS = f"/tmp/kindling-efi-msc-{os.getpid()}.vars"
ERR = f"/tmp/kindling-efi-msc-{os.getpid()}.err"
IMG = f"/tmp/kindling-efi-msc-{os.getpid()}.img"

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


def first_file(cands: list[Path]) -> Path | None:
    for p in cands:
        if p.is_file():
            return p
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


PAGE = b"the volume speaks\n"
# 8.3 alias beside LEAF: LFN `leaf` cannot take the same short name.
SPARK_83 = b"LEAF~1     "


def short_cksum(name11: bytes) -> int:
    s = 0
    for b in name11:
        s = (((s & 1) << 7) + (s >> 1) + b) & 0xFF
    return s


def lfn_entry(long_name: str, name11: bytes) -> bytes:
    """One last-and-first LFN slot (≤13 ASCII chars)."""
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


def plant_fat32(
    img: bytearray,
    part_lba: int = 2048,
    part_secs: int = 2048,
    spark: bytes | None = None,
) -> None:
    """Tiny FAT32: reserved 32, 2×FATSz32 16, root cluster 2, LEAF in cluster 3.

    Optional spark bytes ride LFN `leaf` / 8.3 LEAF~1 in cluster 4 (one sector).
    """
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
    img[fat + 67 : fat + 71] = (0x85C7AA81).to_bytes(4, "little")
    img[fat + 71 : fat + 82] = b"KINDLING   "
    img[fat + 82 : fat + 90] = b"FAT32   "
    img[fat + 510] = 0x55
    img[fat + 511] = 0xAA
    fat1 = fat + 32 * 512
    eoc = ((0, 0x0FFFFFF8), (1, 0x0FFFFFFF), (2, 0x0FFFFFFF), (3, 0x0FFFFFFF))
    if spark:
        eoc = eoc + ((4, 0x0FFFFFFF),)
    for clus, val in eoc:
        off = fat1 + clus * 4
        img[off : off + 4] = (val & 0x0FFFFFFF).to_bytes(4, "little")
    fat2 = fat + (32 + 16) * 512
    img[fat2 : fat2 + 16 * 512] = img[fat1 : fat1 + 16 * 512]
    data = fat + 64 * 512
    img[data : data + 11] = b"LEAF       "
    img[data + 11] = 0x20
    img[data + 26 : data + 28] = (3).to_bytes(2, "little")
    img[data + 28 : data + 32] = len(PAGE).to_bytes(4, "little")
    img[data + 512 : data + 512 + len(PAGE)] = PAGE
    if spark:
        if not spark or len(spark) > 512:
            raise ValueError("spark must fit in one cluster")
        img[data + 32 : data + 64] = lfn_entry("leaf", SPARK_83)
        img[data + 64 : data + 75] = SPARK_83
        img[data + 75] = 0x20
        img[data + 90 : data + 92] = (4).to_bytes(2, "little")
        img[data + 92 : data + 96] = len(spark).to_bytes(4, "little")
        img[data + 1024 : data + 1024 + len(spark)] = spark


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


def main() -> int:
    for p in (MON, SER, VARS, ERR, IMG):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    src = ROOT / "kernel" / "BOOTX64.EFI"
    ld = ROOT / "ld" / "cerne-ld.bin"
    kern = ROOT / "kernel" / "kernel.fw.bin"
    if src.is_file():
        ESP.joinpath("EFI/BOOT").mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src, EFI)
    code = first_file(OVMF_CODE)
    vars_src = first_file(OVMF_VARS)
    if code is None or vars_src is None:
        print("FAIL efi-msc (no OVMF)", file=sys.stderr)
        return 1
    if not EFI.is_file():
        print("FAIL efi-msc (missing BOOTX64.EFI)", file=sys.stderr)
        return 1
    if not ld.is_file() or not kern.is_file():
        print("FAIL efi-msc (missing loader or kernel)", file=sys.stderr)
        return 1
    subprocess.check_call(
        ["nasm", "-f", "bin", "-o", str(ROOT / "spark" / "ingle.bin"), str(ROOT / "spark" / "ingle.asm")],
        cwd=ROOT,
    )
    subprocess.check_call(
        ["nasm", "-f", "bin", "-o", str(ROOT / "spark" / "leaf.bin"), str(ROOT / "spark" / "leaf.asm")],
        cwd=ROOT,
    )
    spark = (ROOT / "spark" / "leaf.bin").read_bytes()
    if not spark or len(spark) > 512:
        print("FAIL efi-msc (leaf spark empty or bigger than one cluster)", file=sys.stderr)
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
            str(ROOT / "kindling-efi-msc-pack.img"),
            "--spark",
            f"ingle={ROOT / 'spark' / 'ingle.bin'}",
            "--cairn-out",
            str(cairn_blob),
        ],
        cwd=ROOT,
    )
    ESP.joinpath("EFI/BOOT").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(cairn_blob, CAIRN)
    img = bytearray(2 * 1024 * 1024)
    # MBR like the Databar: one FAT32 LBA partition at sector 2048.
    img[510] = 0x55
    img[511] = 0xAA
    img[446 + 4] = 0x0C
    img[446 + 8 : 446 + 12] = (2048).to_bytes(4, "little")
    img[446 + 12 : 446 + 16] = (2048).to_bytes(4, "little")
    plant_fat32(img, spark=spark)
    fat = 2048 * 512
    if img[fat + 82 : fat + 87] != b"FAT32" or img[fat + 510] != 0x55:
        print("FAIL efi-msc (planted boot sector is not FAT32)", file=sys.stderr)
        return 1
    data = fat + 64 * 512
    if img[data : data + 11] != b"LEAF       " or img[data + 512 : data + 512 + len(PAGE)] != PAGE:
        print("FAIL efi-msc (planted LEAF is missing)", file=sys.stderr)
        return 1
    if img[data + 64 : data + 75] != SPARK_83 or img[data + 1024 : data + 1024 + len(spark)] != spark:
        print("FAIL efi-msc (planted leaf spark is missing)", file=sys.stderr)
        return 1
    Path(IMG).write_bytes(img)
    shutil.copyfile(vars_src, VARS)
    errf = open(ERR, "wb")
    proc = subprocess.Popen(
        [
            QEMU,
            "-M",
            "q35",
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
            f"if=none,id=kindstick,format=raw,file={IMG}",
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
    try:
        serial = serial_has("kindling: msc", 25)
        t = serial.replace("\r", "")
        if "kindling: usb kbd" not in t:
            print("FAIL efi-msc (no usb kbd — HID lost while scanning MSC)")
            print(serial[-600:])
            try:
                errf.flush()
                print(Path(ERR).read_text(errors="replace")[-400:])
            except OSError:
                pass
            return 1
        if "kindling: msc" not in t:
            print("FAIL efi-msc (no msc — BOT/READ CAPACITY missed)")
            print(serial[-600:])
            try:
                errf.flush()
                print(Path(ERR).read_text(errors="replace")[-400:])
            except OSError:
                pass
            return 1
        if "kindling: fat" not in t:
            print("FAIL efi-msc (no fat — READ(10) LBA 0 missed the boot sector)")
            print(serial[-600:])
            try:
                errf.flush()
                print(Path(ERR).read_text(errors="replace")[-400:])
            except OSError:
                pass
            return 1
        if "the volume speaks" not in t:
            serial = serial_has("the volume speaks", 8)
            t = serial.replace("\r", "")
        if "the volume speaks" not in t:
            print("FAIL efi-msc (glean missed LEAF — the volume never spoke)")
            print(serial[-600:])
            return 1
        if "kindling: ingle ok\ningle\n" not in t:
            serial = serial_has("kindling: ingle ok\ningle\n", 8)
            t = serial.replace("\r", "")
        if "kindling: ingle ok\ningle\n" not in t:
            print("FAIL efi-msc (spark never wrote its name)")
            print(serial[-600:])
            return 1
        deadline = time.time() + 2
        while time.time() < deadline and not Path(MON).exists():
            time.sleep(0.05)
        if not Path(MON).exists():
            print("FAIL efi-msc (no monitor socket)")
            return 1
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2)
        sock.connect(MON)
        banner = read_until(sock, "(qemu)", 2)
        if "(qemu)" not in banner:
            print("FAIL efi-msc (no monitor prompt)")
            print(banner)
            return 1
        sock.sendall(b"sendkey ret\n")
        read_until(sock, "(qemu)", 2)
        serial = serial_has("the fire is lit", 8)
        t = serial.replace("\r", "")
        if "the fire is lit" not in t:
            print("FAIL efi-msc (no greeting — HID reports stolen by BOT)")
            print(serial[-600:])
            sock.close()
            return 1
        sock.sendall(b"sendkey q\n")
        read_until(sock, "(qemu)", 2)
        serial = serial_has("the light remains", 8)
        t = serial.replace("\r", "")
        if "the light remains" not in t:
            print("FAIL efi-msc (no home word — spawn missed the volume spark)")
            print(serial[-600:])
            sock.close()
            return 1
        sock.sendall(b"sendkey q\n")
        read_until(sock, "(qemu)", 2)
        sock.close()
        serial = serial_has("kindling: gleam exit 0", 8)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            print("FAIL efi-msc (no gleam exit 0)")
            print(serial[-600:])
            return 1
        print("ok   efi-msc")
        return 0
    finally:
        errf.close()
        try:
            os.killpg(proc.pid, 9)
        except OSError:
            proc.kill()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.send_signal(9)
        for p in (MON, SER, VARS, ERR, IMG):
            try:
                os.unlink(p)
            except FileNotFoundError:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
