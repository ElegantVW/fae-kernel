#!/usr/bin/env python3
"""Boot EFI Kindling (OVMF). sleep/time must be milliseconds."""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QEMU = os.environ.get("QEMU", "qemu-system-x86_64")
ESP = ROOT / "esp"
EFI = ESP / "EFI" / "BOOT" / "BOOTX64.EFI"
CAIRN = ESP / "EFI" / "BOOT" / "CAIRN"
SER = f"/tmp/kindling-efi-sleep-{os.getpid()}.ser"
VARS = f"/tmp/kindling-efi-sleep-{os.getpid()}.vars"
ERR = f"/tmp/kindling-efi-sleep-{os.getpid()}.err"

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
    for p in (SER, VARS, ERR):
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
        print("FAIL efi-sleep (no OVMF)", file=sys.stderr)
        return 1
    if not EFI.is_file():
        print("FAIL efi-sleep (missing BOOTX64.EFI)", file=sys.stderr)
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
        ],
        stdout=subprocess.DEVNULL,
        stderr=errf,
        cwd=ROOT,
        start_new_session=True,
    )
    try:
        serial = serial_has("kindling: efi slept", 20)
        t = serial.replace("\r", "")
        if "kindling: tick" not in t:
            print("FAIL efi-sleep (no tick — clock never armed)")
            print(serial[-600:])
            try:
                errf.flush()
                print(Path(ERR).read_text(errors="replace")[-400:])
            except OSError:
                pass
            return 1
        if "kindling: efi sleep short" in t:
            print("FAIL efi-sleep (sleep returned before 40 ms)")
            print(serial[-600:])
            return 1
        if "kindling: efi slept" not in t:
            print("FAIL efi-sleep (time did not advance)")
            print(serial[-600:])
            return 1
        print("ok   efi-sleep")
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
        for p in (SER, VARS, ERR):
            try:
                os.unlink(p)
            except FileNotFoundError:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
