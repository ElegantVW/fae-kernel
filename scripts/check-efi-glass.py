#!/usr/bin/env python3
"""Boot EFI Kindling (OVMF), prove GOP ink after CR3 via monitor xp."""
from __future__ import annotations

import os
import re
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
MON = f"/tmp/kindling-efi-glass-{os.getpid()}.mon"
SER = f"/tmp/kindling-efi-glass-{os.getpid()}.ser"
VARS = f"/tmp/kindling-efi-glass-{os.getpid()}.vars"
ERR = f"/tmp/kindling-efi-glass-{os.getpid()}.err"

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

GOP_RE = re.compile(r"kindling: gop 0x([0-9a-fA-F]+) 0x([0-9a-fA-F]+)")


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


def dump(sock: socket.socket, addr: int, n: int) -> str:
    sock.sendall(f"xp /{n}xb {addr:#x}\n".encode())
    return read_until(sock, "(qemu)", 2)


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
        shutil.copyfile(cairn, ESP / "EFI" / "BOOT" / "CAIRN")
    code = first_file(OVMF_CODE)
    vars_src = first_file(OVMF_VARS)
    if code is None or vars_src is None:
        print("FAIL efi-glass (no OVMF)", file=sys.stderr)
        return 1
    if not EFI.is_file():
        print("FAIL efi-glass (missing BOOTX64.EFI)", file=sys.stderr)
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
            "-monitor",
            f"unix:{MON},server,nowait",
        ],
        stdout=subprocess.DEVNULL,
        stderr=errf,
        cwd=ROOT,
        start_new_session=True,
    )
    try:
        serial = serial_has("kindling: well", 20)
        t = serial.replace("\r", "")
        if "kindling: well" not in t:
            print("FAIL efi-glass (no well line)")
            print(serial[-600:])
            try:
                errf.flush()
                print(Path(ERR).read_text(errors="replace")[-400:])
            except OSError:
                pass
            return 1
        m = GOP_RE.search(t)
        if m is None:
            print("FAIL efi-glass (no kindling: gop 0x line)")
            print(serial[-600:])
            return 1
        addr = int(m.group(1), 16)
        pitch = int(m.group(2), 16)
        if addr == 0 or pitch == 0:
            print("FAIL efi-glass (gop addr/pitch zero)")
            return 1
        # show() writes kindling at GOP row 16, x=8.
        off = 16 * 16 * pitch + 8 * 4
        deadline = time.time() + 2
        while time.time() < deadline and not Path(MON).exists():
            time.sleep(0.05)
        if not Path(MON).exists():
            print("FAIL efi-glass (no monitor socket)")
            return 1
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2)
        sock.connect(MON)
        banner = read_until(sock, "(qemu)", 2)
        if "(qemu)" not in banner:
            print("FAIL efi-glass (no monitor prompt)")
            print(banner)
            return 1
        raw = dump(sock, addr + off, 32)
        sock.close()
        hexes = re.findall(r"0x([0-9a-fA-F]{1,2})", raw.replace("\r", ""))
        if len(hexes) < 8:
            print("FAIL efi-glass (xp too short)")
            print(raw)
            return 1
        if all(int(h, 16) == 0 for h in hexes[:16]):
            print("FAIL efi-glass (log row is a black slab)")
            print(raw)
            return 1
        print("ok   efi-glass")
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
