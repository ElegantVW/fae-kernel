#!/usr/bin/env python3
"""Prove plane 2 took the font: FMAP 0x8020 == 1, serial has no 'no font'."""
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
MON = f"/tmp/kindling-font-{os.getpid()}.mon"
SER = f"/tmp/kindling-font-{os.getpid()}.ser"


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


def main() -> int:
    for p in (MON, SER):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    if not FW.is_file() or not IMG.is_file():
        print("FAIL font (missing fw or image)", file=sys.stderr)
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
                if "spark" in serial or "dry" in serial:
                    break
            time.sleep(0.1)
        if "cerne-fw: no font" in serial:
            print("FAIL font (serial said no font)")
            print(serial[-400:])
            return 1
        if not Path(MON).exists():
            print("FAIL font (no monitor socket)")
            return 1
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2)
        sock.connect(MON)
        banner = read_until(sock, "(qemu)", 2)
        if "(qemu)" not in banner:
            print("FAIL font (no monitor prompt)")
            print(banner)
            return 1
        sock.sendall(b"xp /1xb 0x8020\n")
        reply = read_until(sock, "(qemu)", 2)
        sock.close()
        # `00008020: 0x01` or `0x8020: 01`
        got = reply.replace("\r", "")
        if "0x01" not in got and ": 01" not in got.split("8020", 1)[-1]:
            print("FAIL font (0x8020 not 1)")
            print(got)
            return 1
        print("ok   font")
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
