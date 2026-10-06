#!/usr/bin/env python3
"""Boot EFI Kindling with qemu-xhci + usb-kbd. Enumerate, then Enter."""
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
MON = f"/tmp/kindling-efi-usb-{os.getpid()}.mon"
SER = f"/tmp/kindling-efi-usb-{os.getpid()}.ser"
VARS = f"/tmp/kindling-efi-usb-{os.getpid()}.vars"
ERR = f"/tmp/kindling-efi-usb-{os.getpid()}.err"

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
    for p in (MON, SER, VARS, ERR):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    src = ROOT / "kernel" / "BOOTX64.EFI"
    if src.is_file():
        ESP.joinpath("EFI/BOOT").mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src, EFI)
    cairn = ROOT / "spark" / "cairn.bin"
    if cairn.is_file():
        shutil.copyfile(cairn, CAIRN)
    code = first_file(OVMF_CODE)
    vars_src = first_file(OVMF_VARS)
    if code is None or vars_src is None:
        print("FAIL efi-usb (no OVMF)", file=sys.stderr)
        return 1
    if not EFI.is_file() or not CAIRN.is_file():
        print("FAIL efi-usb (missing BOOTX64.EFI or CAIRN)", file=sys.stderr)
        return 1
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
            "-device",
            "qemu-xhci,id=xhci",
            "-device",
            "usb-kbd,bus=xhci.0",
            "-monitor",
            f"unix:{MON},server,nowait",
        ],
        stdout=subprocess.DEVNULL,
        stderr=errf,
        cwd=ROOT,
        start_new_session=True,
    )
    try:
        serial = serial_has("kindling: usb kbd", 20)
        t = serial.replace("\r", "")
        if "kindling: usb kbd" not in t:
            print("FAIL efi-usb (no usb kbd — enumeration missed)")
            print(serial[-600:])
            try:
                errf.flush()
                print(Path(ERR).read_text(errors="replace")[-400:])
            except OSError:
                pass
            return 1
        if "kindling: ingle ok\ningle\n" not in t:
            # ingle may still be printing; wait a little more
            serial = serial_has("kindling: ingle ok\ningle\n", 8)
            t = serial.replace("\r", "")
        if "kindling: ingle ok\ningle\n" not in t:
            print("FAIL efi-usb (spark never wrote its name)")
            print(serial[-600:])
            return 1
        deadline = time.time() + 2
        while time.time() < deadline and not Path(MON).exists():
            time.sleep(0.05)
        if not Path(MON).exists():
            print("FAIL efi-usb (no monitor socket)")
            return 1
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2)
        sock.connect(MON)
        banner = read_until(sock, "(qemu)", 2)
        if "(qemu)" not in banner:
            print("FAIL efi-usb (no monitor prompt)")
            print(banner)
            return 1
        sock.sendall(b"sendkey ret\n")
        read_until(sock, "(qemu)", 2)
        serial = serial_has("the fire is lit", 8)
        t = serial.replace("\r", "")
        if "the fire is lit" not in t:
            print("FAIL efi-usb (no greeting)")
            print(serial[-600:])
            sock.close()
            return 1
        sock.sendall(b"sendkey q\n")
        read_until(sock, "(qemu)", 2)
        sock.close()
        serial = serial_has("kindling: gleam exit 0", 8)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            print("FAIL efi-usb (no gleam exit 0)")
            print(serial[-600:])
            return 1
        print("ok   efi-usb")
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
        for p in (MON, SER, VARS, ERR):
            try:
                os.unlink(p)
            except FileNotFoundError:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
