#!/usr/bin/env python3
"""BIOS ingle + keeper: first boot creates a keeper; second boot chooses.

Cairn has ingle, keeper, wax hand, and wax hands/twin (716 bytes). QEMU
CPU is qemu64,+aes,+rdrand. First boot: name gil (gix, backspace, l),
word tinder twice. Keys echo; the word is stars. Enter lights the fire,
q leaves. Boot2 is the hall: 1 gil, the word, fire. The word never
appears on serial.
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
WORD = "tinder"
CPU = "qemu64,+aes,+rdrand"


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
            "-cpu",
            CPU,
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
    serial = serial_has(want, 8)
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


def fail(msg: str, serial: str) -> int:
    print(f"FAIL ingle-keeper ({msg})")
    print(serial[-500:])
    return 1


def main() -> int:
    if not FW.is_file() or not IMG.is_file():
        print("FAIL ingle-keeper (missing fw or image)", file=sys.stderr)
        return 1
    proc = None
    sock = None
    try:
        proc, sock, serial = boot("kindling: ingle ok\ningle\n")
        if "kindling: ingle ok\ningle\n" not in serial:
            return fail("spark never wrote its name", serial)
        serial = serial_has("who keeps this fire", 4)
        t = serial.replace("\r", "")
        if "who keeps this fire" not in t:
            return fail("no keeper prompt on first boot", t)
        send_keys(sock, "gix\bl\n")
        serial = serial_has("speak the word", 4)
        t = serial.replace("\r", "")
        mark = t.find("who keeps this fire")
        if mark < 0:
            return fail("prompt vanished", t)
        echoed = crush(t[mark:])
        if "gix" not in t:
            return fail("name keys never echoed", t)
        if NAME not in echoed:
            return fail("echo+backspace did not land gil", t)
        if "speak the word" not in t:
            return fail("no word prompt on first boot", t)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has("speak it again", 4)
        t = serial.replace("\r", "")
        if "speak it again" not in t:
            return fail("no confirm prompt", t)
        after_word = t[t.find("speak the word") :]
        if WORD in after_word:
            return fail("the word leaked on serial", t)
        if "*" not in after_word:
            return fail("word did not echo as stars", t)
        send_keys(sock, f"{WORD}\n")
        serial = serial_has("speak it again", 4)
        t = serial.replace("\r", "")
        again = t.find("speak it again")
        end = time.time() + 4
        greeted = False
        while time.time() < end:
            t = Path(SER).read_bytes().decode(errors="replace").replace("\r", "")
            tail = t[again:] if again >= 0 else t
            if f"\n{NAME}\n" in tail or tail.endswith(f"{NAME}\n"):
                greeted = True
                break
            time.sleep(0.05)
        if not greeted:
            return fail("ingle did not greet after enlist", t)
        send_keys(sock, "\n")
        serial = serial_has("the fire is lit", 4)
        t = serial.replace("\r", "")
        if "the fire is lit" not in t:
            return fail("no greeting on first boot", t)
        send_keys(sock, "q")
        sock.close()
        sock = None
        serial = serial_has("kindling: gleam exit 0", 4)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            return fail("no gleam exit 0 on first boot", t)
        if WORD in t:
            return fail("the word leaked on serial", t)
        blob = IMG.read_bytes()
        if WORD.encode() in blob:
            return fail("the word is on the disk", t)
        stop(proc, None)
        proc = None

        proc, sock, serial = boot("kindling: ingle ok\ningle\n")
        serial = serial_has("who keeps this fire", 4)
        t = serial.replace("\r", "")
        if "who keeps this fire" not in t:
            return fail("no hall on second boot", t)
        if "1 gil" not in t and "1 gil\n" not in crush(t):
            return fail("hall did not list gil", t)
        send_keys(sock, "1")
        serial = serial_has("speak the word", 4)
        t = serial.replace("\r", "")
        if "speak the word" not in t:
            return fail("no word prompt on choose", t)
        send_keys(sock, f"{WORD}\n")
        end = time.time() + 4
        greeted = False
        while time.time() < end:
            t = Path(SER).read_bytes().decode(errors="replace").replace("\r", "")
            if t.count(NAME) >= 1 and "the book is split" not in t:
                # wait for ingle to greet after choose
                mark = t.rfind("speak the word")
                if mark >= 0 and NAME in t[mark:]:
                    greeted = True
                    break
            time.sleep(0.05)
        if not greeted:
            return fail("second boot did not greet after choose", t)
        send_keys(sock, "\n")
        serial = serial_has("the fire is lit", 4)
        t = serial.replace("\r", "")
        if "the fire is lit" not in t:
            return fail("no fire after choose", t)
        send_keys(sock, "q")
        sock.close()
        sock = None
        serial = serial_has("kindling: gleam exit 0", 4)
        t = serial.replace("\r", "")
        if "kindling: gleam exit 0" not in t:
            return fail("no gleam exit 0 on second boot", t)
        if WORD in t:
            return fail("the word leaked on serial", t)
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
