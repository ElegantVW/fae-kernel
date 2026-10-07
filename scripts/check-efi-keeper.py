#!/usr/bin/env python3
"""EFI keeper: ingle inks the volume hand; a second boot greets it.

Happy BOOTX64. Cairn has only ingle. FAT32 at LBA 2048 is KINDLING
`85C7-AA81` with wax LFN `hand` (32 zeros) and LFN `keeper`. Keys echo:
type gix, backspace, l so the glass shows gil. Enter lights the fire,
q leaves. Boot2 greets gil and never asks.
Does not touch the live Databar.
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
HAND_LEN = 32
HAND_83 = b"HAND       "
KEEPER_83 = b"KEEPER     "
KINDLING_VOL = 0x85C7AA81
PART_LBA = 2048


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


def plant_fat32(img: bytearray, spark: bytes, part_lba: int = PART_LBA, part_secs: int = 2048) -> None:
    """Tiny FAT32: wax hand in cluster 3, keeper spark in cluster 4."""
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
    for clus, val in ((0, 0x0FFFFFF8), (1, 0x0FFFFFFF), (2, 0x0FFFFFFF), (3, 0x0FFFFFFF), (4, 0x0FFFFFFF)):
        off = fat1 + clus * 4
        img[off : off + 4] = (val & 0x0FFFFFFF).to_bytes(4, "little")
    fat2 = fat + (32 + 16) * 512
    img[fat2 : fat2 + 16 * 512] = img[fat1 : fat1 + 16 * 512]
    data = fat + 64 * 512
    img[data : data + 32] = lfn_entry("hand", HAND_83)
    img[data + 32 : data + 43] = HAND_83
    img[data + 43] = 0x20
    img[data + 58 : data + 60] = (3).to_bytes(2, "little")
    img[data + 60 : data + 64] = HAND_LEN.to_bytes(4, "little")
    img[data + 64 : data + 96] = lfn_entry("keeper", KEEPER_83)
    img[data + 96 : data + 107] = KEEPER_83
    img[data + 107] = 0x20
    img[data + 122 : data + 124] = (4).to_bytes(2, "little")
    img[data + 124 : data + 128] = len(spark).to_bytes(4, "little")
    img[data + 1024 : data + 1024 + len(spark)] = spark


def hand_off() -> int:
    return PART_LBA * 512 + 64 * 512 + 512


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
    if not spark or len(spark) > 512:
        print("FAIL efi-keeper (keeper spark empty or bigger than one cluster)", file=sys.stderr)
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
    img = bytearray(2 * 1024 * 1024)
    img[510] = 0x55
    img[511] = 0xAA
    img[446 + 4] = 0x0C
    img[446 + 8 : 446 + 12] = PART_LBA.to_bytes(4, "little")
    img[446 + 12 : 446 + 16] = (2048).to_bytes(4, "little")
    plant_fat32(img, spark)
    fat = PART_LBA * 512
    if int.from_bytes(img[fat + 67 : fat + 71], "little") != KINDLING_VOL:
        print("FAIL efi-keeper (planted volume serial is not KINDLING)", file=sys.stderr)
        return 1
    Path(IMG).write_bytes(img)
    shutil.copyfile(vars_src, VARS)

    proc = boot_once(code)
    sock = None
    try:
        serial = serial_has("kindling: ingle ok\ningle\n", 25)
        t = serial.replace("\r", "")
        if "kindling: fat" not in t:
            print("FAIL efi-keeper (no fat)")
            print(serial[-600:])
            return 1
        if "kindling: ingle ok\ningle\n" not in t:
            print("FAIL efi-keeper (spark never wrote its name)")
            print(serial[-600:])
            return 1
        serial = serial_has("who keeps this fire", 8)
        t = serial.replace("\r", "")
        if "who keeps this fire" not in t:
            print("FAIL efi-keeper (no keeper prompt on first boot)")
            print(serial[-600:])
            return 1
        sock = monitor()
        if sock is None:
            print("FAIL efi-keeper (no monitor)")
            return 1
        send_keys(sock, "gix\bl\n")
        serial = serial_has(NAME, 8)
        t = serial.replace("\r", "")
        mark = t.find("who keeps this fire")
        if mark < 0:
            print("FAIL efi-keeper (prompt vanished)")
            print(serial[-600:])
            return 1
        echoed = crush(t[mark:])
        if "gix" not in t:
            print("FAIL efi-keeper (name keys never echoed)")
            print(serial[-600:])
            return 1
        if NAME not in echoed:
            print("FAIL efi-keeper (echo+backspace did not land gil)")
            print(serial[-600:])
            return 1
        send_keys(sock, "\n")
        serial = serial_has("the fire is lit", 8)
        t = serial.replace("\r", "")
        if "the fire is lit" not in t:
            print("FAIL efi-keeper (no greeting on first boot)")
            print(serial[-600:])
            return 1
        send_keys(sock, "q")
        sock.close()
        sock = None
        serial = serial_has("kindling: gleam exit 0", 8)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            print("FAIL efi-keeper (no gleam exit 0 on first boot)")
            print(serial[-600:])
            return 1
    finally:
        if sock is not None:
            sock.close()
        stop(proc)

    landed = Path(IMG).read_bytes()
    off = hand_off()
    if landed[off : off + len(NAME)] != NAME.encode() or landed[off + len(NAME)] != 0:
        print("FAIL efi-keeper (WRITE(10) never landed the name)")
        return 1

    proc = boot_once(code)
    sock = None
    try:
        serial = serial_has(f"ingle\n{NAME}\n", 25)
        t = serial.replace("\r", "")
        if f"ingle\n{NAME}\n" not in t:
            print("FAIL efi-keeper (second boot did not greet the name)")
            print(serial[-600:])
            return 1
        if "who keeps this fire" in t:
            print("FAIL efi-keeper (re-asked on second boot — ink never landed)")
            print(serial[-600:])
            return 1
        sock = monitor()
        if sock is None:
            print("FAIL efi-keeper (no monitor on second boot)")
            return 1
        send_keys(sock, "\n")
        serial = serial_has("the fire is lit", 8)
        t = serial.replace("\r", "")
        if "the fire is lit" not in t:
            print("FAIL efi-keeper (no greeting on second boot)")
            print(serial[-600:])
            return 1
        if "who keeps this fire" in t:
            print("FAIL efi-keeper (re-asked on second boot — ink never landed)")
            print(serial[-600:])
            return 1
        send_keys(sock, "q")
        sock.close()
        sock = None
        serial = serial_has("kindling: gleam exit 0", 8)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            print("FAIL efi-keeper (no gleam exit 0 on second boot)")
            print(serial[-600:])
            return 1
        print("ok   efi-keeper")
        return 0
    finally:
        if sock is not None:
            sock.close()
        stop(proc)


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
