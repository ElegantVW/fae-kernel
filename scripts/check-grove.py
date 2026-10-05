#!/usr/bin/env python3
"""Prove the glass shows Grove: VGA B8000 title row is G r o v e in parchment."""
from __future__ import annotations

import os
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QEMU = os.environ.get("QEMU", "qemu-system-x86_64")
FW = ROOT / "fw" / "cerne-fw.bin"
IMG = ROOT / "kindling.img"
MON = f"/tmp/kindling-grove-{os.getpid()}.mon"
SER = f"/tmp/kindling-grove-{os.getpid()}.ser"

# Row 14, col 0: title "Grove" as char+attr words (parchment 0x07 on night).
TITLE_ADDR = 0xB8000 + 14 * 160
# Row 8, col 0: Grove sigil starts with backslash, violet 0x05.
SIGIL_ADDR = 0xB8000 + 8 * 160


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


def dump(sock: socket.socket, addr: int, n: int) -> str:
    sock.sendall(f"xp /{n}xb {addr:#x}\n".encode())
    return read_until(sock, "(qemu)", 2)


def main() -> int:
    for p in (MON, SER):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    if not FW.is_file() or not IMG.is_file():
        print("FAIL grove (missing fw or image)", file=sys.stderr)
        return 1
    proc = subprocess.Popen(
        [
            QEMU,
            "-M",
            "pc",
            "-m",
            "256M",
            "-bios",
            str(FW),
            "-drive",
            f"if=ide,format=raw,file={IMG}",
            "-display",
            "none",
            "-serial",
            f"file:{SER}",
            "-no-reboot",
            "-no-shutdown",
            "-monitor",
            f"unix:{MON},server,nowait",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        deadline = time.time() + 5
        serial = ""
        while time.time() < deadline:
            if Path(SER).is_file():
                serial = Path(SER).read_bytes().decode(errors="replace")
                if "well" in serial or "dry" in serial:
                    break
            time.sleep(0.1)
        if "well" not in serial:
            print("FAIL grove (no well line)")
            print(serial[-400:])
            return 1
        deadline = time.time() + 2
        while time.time() < deadline and not Path(MON).exists():
            time.sleep(0.05)
        if not Path(MON).exists():
            print("FAIL grove (no monitor socket)")
            return 1
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2)
        sock.connect(MON)
        banner = read_until(sock, "(qemu)", 2)
        if "(qemu)" not in banner:
            print("FAIL grove (no monitor prompt)")
            print(banner)
            return 1
        title = dump(sock, TITLE_ADDR, 10)
        sigil = dump(sock, SIGIL_ADDR, 4)
        sock.close()
        t = title.replace("\r", "").lower()
        s = sigil.replace("\r", "").lower()
        # G 0x47, parchment 0x07, r 0x72, o 0x6f, v 0x76, e 0x65
        want = ("0x47", "0x07", "0x72", "0x07", "0x6f", "0x07", "0x76", "0x07", "0x65")
        if not all(w in t for w in want):
            print("FAIL grove (title row is not Grove)")
            print(title)
            return 1
        if "0x5c" not in s and "0x5C" not in sigil:
            print("FAIL grove (sigil row has no backslash)")
            print(sigil)
            return 1
        print("ok   grove")
        return 0
    finally:
        proc.kill()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.send_signal(9)
        for p in (MON, SER):
            try:
                os.unlink(p)
            except FileNotFoundError:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
