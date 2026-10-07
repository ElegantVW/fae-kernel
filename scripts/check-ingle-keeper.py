#!/usr/bin/env python3
"""BIOS ingle + keeper: first boot inks the hand; second boot greets it.

Cairn has ingle, keeper, and a 32-byte wax hand. No leaf. Type gil, Enter
lights the fire, q leaves. Boot2 greets gil and never asks who keeps this fire.
"""
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
IMG = Path(os.environ.get("INGLE_KEEPER_IMG", ROOT / "kindling-ingle-keeper.img"))
MON = f"/tmp/kindling-ingle-keeper-{os.getpid()}.mon"
SER = f"/tmp/kindling-ingle-keeper-{os.getpid()}.ser"

NAME = "gil"


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
        else:
            sock.sendall(f"sendkey {ch}\n".encode())
        read_until(sock, "(qemu)", 2)


def boot(want: str) -> tuple[subprocess.Popen, socket.socket, str]:
    try:
        os.unlink(SER)
    except FileNotFoundError:
        pass
    try:
        os.unlink(MON)
    except FileNotFoundError:
        pass
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
        start_new_session=True,
    )
    serial = serial_has(want, 6)
    deadline = time.time() + 2
    while time.time() < deadline and not Path(MON).exists():
        time.sleep(0.05)
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.settimeout(2)
    sock.connect(MON)
    banner = read_until(sock, "(qemu)", 2)
    if "(qemu)" not in banner:
        sock.close()
        raise RuntimeError("no monitor prompt")
    return proc, sock, serial.replace("\r", "")


def stop(proc: subprocess.Popen, sock: socket.socket | None) -> None:
    if sock is not None:
        try:
            sock.close()
        except OSError:
            pass
    try:
        os.killpg(proc.pid, 9)
    except OSError:
        proc.kill()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.send_signal(9)


def main() -> int:
    if not FW.is_file() or not IMG.is_file():
        print("FAIL ingle-keeper (missing fw or image)", file=sys.stderr)
        return 1
    proc = None
    sock = None
    try:
        proc, sock, serial = boot("kindling: ingle ok\ningle\n")
        if "kindling: ingle ok\ningle\n" not in serial:
            print("FAIL ingle-keeper (spark never wrote its name)")
            print(serial[-400:])
            return 1
        serial = serial_has("who keeps this fire", 4)
        t = serial.replace("\r", "")
        if "who keeps this fire" not in t:
            print("FAIL ingle-keeper (no keeper prompt on first boot)")
            print(serial[-400:])
            return 1
        send_keys(sock, NAME + "\n")
        serial = serial_has(f"{NAME}\n", 4)
        t = serial.replace("\r", "")
        if f"\n{NAME}\n" not in t and not t.endswith(f"{NAME}\n"):
            # ingle writes the name after keeper smoors
            serial = serial_has(NAME, 4)
            t = serial.replace("\r", "")
        if NAME not in t:
            print("FAIL ingle-keeper (name never spoke on first boot)")
            print(serial[-400:])
            return 1
        send_keys(sock, "\n")
        serial = serial_has("the fire is lit", 4)
        t = serial.replace("\r", "")
        if "the fire is lit" not in t:
            print("FAIL ingle-keeper (no greeting on first boot)")
            print(serial[-400:])
            return 1
        send_keys(sock, "q")
        sock.close()
        sock = None
        serial = serial_has("kindling: gleam exit 0", 4)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            print("FAIL ingle-keeper (no gleam exit 0 on first boot)")
            print(serial[-400:])
            return 1
        stop(proc, None)
        proc = None

        proc, sock, serial = boot("kindling: ingle ok\ningle\n")
        serial = serial_has(f"ingle\n{NAME}\n", 4)
        t = serial.replace("\r", "")
        if f"ingle\n{NAME}\n" not in t:
            print("FAIL ingle-keeper (second boot did not greet the name)")
            print(serial[-400:])
            return 1
        if "who keeps this fire" in t:
            print("FAIL ingle-keeper (re-asked on second boot — ink never landed)")
            print(serial[-400:])
            return 1
        send_keys(sock, "\n")
        serial = serial_has("the fire is lit", 4)
        t = serial.replace("\r", "")
        if "the fire is lit" not in t:
            print("FAIL ingle-keeper (no greeting on second boot)")
            print(serial[-400:])
            return 1
        if "who keeps this fire" in t:
            print("FAIL ingle-keeper (re-asked on second boot — ink never landed)")
            print(serial[-400:])
            return 1
        send_keys(sock, "q")
        sock.close()
        sock = None
        serial = serial_has("kindling: gleam exit 0", 4)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            print("FAIL ingle-keeper (no gleam exit 0 on second boot)")
            print(serial[-400:])
            return 1
        print("ok   ingle-keeper")
        return 0
    except (OSError, RuntimeError) as e:
        print(f"FAIL ingle-keeper ({e})")
        return 1
    finally:
        if proc is not None:
            stop(proc, sock)
        for p in (MON, SER):
            try:
                os.unlink(p)
            except FileNotFoundError:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
