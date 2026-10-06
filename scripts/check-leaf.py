#!/usr/bin/env python3
"""Boot leaf, send q, prove the spark told the page."""
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
IMG = Path(os.environ.get("LEAF_IMG", ROOT / "kindling-leaf.img"))
MON = f"/tmp/kindling-leaf-{os.getpid()}.mon"
SER = f"/tmp/kindling-leaf-{os.getpid()}.ser"


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
    for p in (MON, SER):
        try:
            os.unlink(p)
        except FileNotFoundError:
            pass
    if not FW.is_file() or not IMG.is_file():
        print("FAIL leaf (missing fw or image)", file=sys.stderr)
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
        serial = serial_has("kindling: leaf ok", 6)
        t = serial.replace("\r", "")
        if "kindling: leaf ok" not in t:
            print("FAIL leaf (spark never kindled)")
            print(serial[-400:])
            return 1
        if "the volume speaks" not in t:
            serial = serial_has("the volume speaks", 4)
            t = serial.replace("\r", "")
        if "the volume speaks" not in t:
            print("FAIL leaf (glean missed LEAF)")
            print(serial[-400:])
            return 1
        deadline = time.time() + 2
        while time.time() < deadline and not Path(MON).exists():
            time.sleep(0.05)
        if not Path(MON).exists():
            print("FAIL leaf (no monitor socket)")
            return 1
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2)
        sock.connect(MON)
        banner = read_until(sock, "(qemu)", 2)
        if "(qemu)" not in banner:
            print("FAIL leaf (no monitor prompt)")
            print(banner)
            return 1
        sock.sendall(b"sendkey q\n")
        read_until(sock, "(qemu)", 2)
        sock.close()
        serial = serial_has("kindling: gleam exit 0", 4)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            print("FAIL leaf (no gleam exit 0)")
            print(serial[-400:])
            return 1
        print("ok   leaf")
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
