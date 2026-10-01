#!/bin/sh
# test-vm — Kindling test VM: build (if asked), boot, judge the transcript.
#
# usage:
#   test-vm happy|house|ring3|reclaim   build + boot the known image, check it
#   test-vm <image> <want>...           boot any image; every want must appear
#
# Logs land in test-logs/<case>.log. Prints PASS/FAIL. Exit 0/1.
# The VM is throwaway: pc, 256M, our firmware, serial to the log. No window.
set -eu
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="${HOME}/.cargo/bin:$PATH"
QEMU="${QEMU:-qemu-system-x86_64}"
FW=fw/cerne-fw.bin
LOGDIR=test-logs
mkdir -p "$LOGDIR"

boot() {
    img=$1
    log=$2
    timeout --foreground --signal=KILL 4 \
        "$QEMU" -M pc -m 256M -bios "$FW" \
        -drive if=ide,format=raw,file="$img" \
        -display none -serial stdio -no-reboot -no-shutdown \
        >"$log" 2>/dev/null || true
    tr -d '\r' <"$log" >"$log.clean" && mv "$log.clean" "$log"
}

judge() {
    log=$1
    shift
    fail=0
    for w in "$@"; do
        if ! grep -F -q "$w" "$log"; then
            echo "MISS: $w"
            fail=1
        fi
    done
    return $fail
}

run_case() {
    name=$1
    img=$2
    log="$LOGDIR/$name.log"
    shift 2
    echo "---- vm $name ($img) ----"
    boot "$img" "$log"
    if judge "$log" "$@"; then
        echo "PASS $name"
    else
        echo "FAIL $name — see $log"
        tail -8 "$log"
        exit 1
    fi
}

case "${1:-}" in
happy)
    make image >/dev/null
    run_case happy kindling.img \
        "cerne-fw" "cerne-ld" "fae-kernel" \
        "kindling: still only a spark" "well 256 MiB"
    ;;
house)
    make -C kernel house-bin >/dev/null
    python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
        --kernel kernel/kernel.house.bin --out kindling-house.img >/dev/null
    run_case house kindling-house.img \
        "kindling: house ok" "well 256 MiB"
    ;;
ring3)
    make -C kernel ring3-bin >/dev/null
    python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
        --kernel kernel/kernel.ring3.bin --out kindling-ring3.img >/dev/null
    run_case ring3 kindling-ring3.img \
        "kindling: house ok" "well 256 MiB" \
        "kindling: init ok" "kindling: init slept" "kindling: gleam exit 0"
    ;;
reclaim)
    make -C kernel reclaim-bin >/dev/null
    python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
        --kernel kernel/kernel.reclaim.bin --out kindling-reclaim.img >/dev/null
    run_case reclaim kindling-reclaim.img \
        "kindling: house ok" "kindling: reclaim ok" "well 256 MiB"
    ;;
"")
    echo "usage: test-vm happy|house|ring3|reclaim|<image> <want>..." >&2
    exit 2
    ;;
*)
    img=$1
    shift
    if [ "$#" -eq 0 ]; then
        echo "usage: test-vm <image> <want>..." >&2
        exit 2
    fi
    name=$(basename "$img" .img)
    log="$LOGDIR/$name.log"
    echo "---- vm $name ($img) ----"
    boot "$img" "$log"
    if judge "$log" "$@"; then
        echo "PASS $name"
    else
        echo "FAIL $name — see $log"
        tail -8 "$log"
        exit 1
    fi
    ;;
esac
