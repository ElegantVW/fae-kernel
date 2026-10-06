#!/usr/bin/env python3
"""EFI leaf: cairn has the spark, FAT has LEAF, HID sends q.

Happy BOOTX64 stays ingle. This recast is leaf-test (`BOOTX64.LEAF.EFI`).
The spark gleans LEAF from the volume root (no LEAF record in the cairn).
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
EFI_SRC = ROOT / "kernel" / "BOOTX64.LEAF.EFI"
EFI = ESP / "EFI" / "BOOT" / "BOOTX64.EFI"
CAIRN = ESP / "EFI" / "BOOT" / "CAIRN"
MON = f"/tmp/kindling-efi-leaf-{os.getpid()}.mon"
SER = f"/tmp/kindling-efi-leaf-{os.getpid()}.ser"
VARS = f"/tmp/kindling-efi-leaf-{os.getpid()}.vars"
ERR = f"/tmp/kindling-efi-leaf-{os.getpid()}.err"
IMG = f"/tmp/kindling-efi-leaf-{os.getpid()}.img"

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

PAGE = b"the volume speaks\n"


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


def plant_fat32(img: bytearray, part_lba: int = 2048, part_secs: int = 2048) -> None:
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
    img[fat + 82 : fat + 90] = b"FAT32   "
    img[fat + 510] = 0x55
    img[fat + 511] = 0xAA
    fat1 = fat + 32 * 512
    for clus, val in ((0, 0x0FFFFFF8), (1, 0x0FFFFFFF), (2, 0x0FFFFFFF), (3, 0x0FFFFFFF)):
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
    code = first_file(OVMF_CODE)
    vars_src = first_file(OVMF_VARS)
    if code is None or vars_src is None:
        print("FAIL efi-leaf (no OVMF)", file=sys.stderr)
        return 1
    if not EFI_SRC.is_file():
        print("FAIL efi-leaf (missing BOOTX64.LEAF.EFI)", file=sys.stderr)
        return 1
    subprocess.check_call(["nasm", "-f", "bin", "-o", str(ROOT / "spark" / "leaf.bin"), str(ROOT / "spark" / "leaf.asm")])
    cairn_blob = ROOT / "spark" / "cairn-leaf.bin"
    subprocess.check_call(
        [
            "python3",
            str(ROOT / "scripts" / "mkimg.py"),
            "--loader",
            str(ROOT / "ld" / "cerne-ld.bin"),
            "--kernel",
            str(ROOT / "kernel" / "kernel.fw.bin"),
            "--out",
            str(ROOT / "kindling-efi-leaf-pack.img"),
            "--spark",
            f"leaf={ROOT / 'spark' / 'leaf.bin'}",
            "--cairn-out",
            str(cairn_blob),
        ],
        cwd=ROOT,
    )
    ESP.joinpath("EFI/BOOT").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(EFI_SRC, EFI)
    shutil.copyfile(cairn_blob, CAIRN)
    img = bytearray(2 * 1024 * 1024)
    img[510] = 0x55
    img[511] = 0xAA
    img[446 + 4] = 0x0C
    img[446 + 8 : 446 + 12] = (2048).to_bytes(4, "little")
    img[446 + 12 : 446 + 16] = (2048).to_bytes(4, "little")
    plant_fat32(img)
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
        serial = serial_has("kindling: leaf ok", 25)
        t = serial.replace("\r", "")
        if "kindling: usb kbd" not in t:
            print("FAIL efi-leaf (no usb kbd)")
            print(serial[-600:])
            return 1
        if "kindling: fat" not in t:
            print("FAIL efi-leaf (no fat)")
            print(serial[-600:])
            return 1
        if "kindling: leaf ok" not in t:
            print("FAIL efi-leaf (spark never kindled)")
            print(serial[-600:])
            return 1
        if "the volume speaks" not in t:
            serial = serial_has("the volume speaks", 8)
            t = serial.replace("\r", "")
        if "the volume speaks" not in t:
            print("FAIL efi-leaf (glean missed LEAF on the volume)")
            print(serial[-600:])
            return 1
        deadline = time.time() + 2
        while time.time() < deadline and not Path(MON).exists():
            time.sleep(0.05)
        if not Path(MON).exists():
            print("FAIL efi-leaf (no monitor socket)")
            return 1
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2)
        sock.connect(MON)
        banner = read_until(sock, "(qemu)", 2)
        if "(qemu)" not in banner:
            print("FAIL efi-leaf (no monitor prompt)")
            print(banner)
            return 1
        sock.sendall(b"sendkey q\n")
        read_until(sock, "(qemu)", 2)
        sock.close()
        serial = serial_has("kindling: gleam exit 0", 8)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            print("FAIL efi-leaf (no gleam exit 0)")
            print(serial[-600:])
            return 1
        print("ok   efi-leaf")
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
        try:
            os.unlink(ROOT / "kindling-efi-leaf-pack.img")
        except FileNotFoundError:
            pass


if __name__ == "__main__":
    raise SystemExit(main())
