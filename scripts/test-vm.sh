#!/bin/sh
# test-vm — Kindling test VM: build (if asked), boot, judge the transcript.
#
# usage:
#   test-vm happy|house|ring3|reclaim|tale|spawn|splanc|ingle|leaf   build + boot the known image
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
tale)
    nasm -f bin -o spark/tale.bin spark/tale.asm
    make -C kernel tale-bin >/dev/null
    python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
        --kernel kernel/kernel.tale.bin --out kindling-tale.img \
        --spark tale=spark/tale.bin --leaf first-leaf=spark/first-leaf.txt \
        --leaf slate=spark/slate.txt >/dev/null
    run_case tale kindling-tale.img \
        "kindling remembers the reset" "stowed" "ink holds" "kindling: gleam exit 0"
    # Same disk, second boot: ink must read kept, never re-stowed.
    log="test-logs/tale2.log"
    timeout --foreground --signal=KILL 4 \
        "$QEMU" -M pc -m 256M -bios fw/cerne-fw.bin \
        -drive if=ide,format=raw,file=kindling-tale.img \
        -display none -serial stdio -no-reboot -no-shutdown \
        >"$log" 2>/dev/null || true
    tr -d '\r' <"$log" >"$log.clean" && mv "$log.clean" "$log"
    if grep -F -q "stowed" "$log"; then
        echo "FAIL tale2 (re-stowed — ink never landed)"
        tail -8 "$log"
        exit 1
    fi
    echo "---- vm tale2 (same disk) ----"
    if grep -F -q "kept" "$log" && grep -F -q "ink holds" "$log"; then
        echo "PASS tale2"
    else
        echo "FAIL tale2 — see $log"
        tail -8 "$log"
        exit 1
    fi
    # Recast pristine so the next run starts from wax.
    python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
        --kernel kernel/kernel.tale.bin --out kindling-tale.img \
        --spark tale=spark/tale.bin --leaf first-leaf=spark/first-leaf.txt \
        --leaf slate=spark/slate.txt >/dev/null
    ;;
spawn)
    nasm -f bin -o spark/wick.bin spark/wick.asm
    nasm -f bin -o spark/ember.bin spark/ember.asm
    make -C kernel spawn-bin >/dev/null
    python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
        --kernel kernel/kernel.spawn.bin --out kindling-spawn.img \
        --spark ember=spark/ember.bin --spark wick=spark/wick.bin \
        --leaf second-leaf=spark/second-leaf.txt >/dev/null
    run_case spawn kindling-spawn.img \
        "kindling: house ok" "kindling: spawn ok" \
        "the cup is its own" "stayed" "kindling: gleam exit 0"
    ;;
splanc)
    nasm -f bin -o spark/ember.bin spark/ember.asm
    nasm -f bin -o spark/splanc.bin spark/splanc.asm
    make -C kernel spawn-bin >/dev/null
    python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
        --kernel kernel/kernel.spawn.bin --out kindling-splanc.img \
        --spark ember=spark/ember.bin --spark splanc=spark/splanc.bin >/dev/null
    run_case splanc kindling-splanc.img \
        "kindling: house ok" "kindling: spawn ok" \
        "kindling: trap 6" "the spark went out" "kindling: gleam exit 1"
    if grep -F -q "stayed" "$LOGDIR/splanc.log"; then
        echo "FAIL splanc (stayed — a lie)"
        tail -8 "$LOGDIR/splanc.log"
        exit 1
    fi
    ;;
ingle)
    nasm -f bin -o spark/ingle.bin spark/ingle.asm
    make -C kernel ingle-bin >/dev/null
    python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
        --kernel kernel/kernel.ingle.bin --out kindling-ingle.img \
        --spark ingle=spark/ingle.bin >/dev/null
    python3 scripts/check-ingle.py
    ;;
leaf)
    nasm -f bin -o spark/leaf.bin spark/leaf.asm
    make -C kernel leaf-bin >/dev/null
    python3 scripts/mkimg.py --loader ld/cerne-ld.bin \
        --kernel kernel/kernel.leaf.bin --out kindling-leaf.img \
        --spark leaf=spark/leaf.bin --leaf LEAF=spark/LEAF.txt >/dev/null
    python3 scripts/check-leaf.py
    ;;
"")
    echo "usage: test-vm happy|house|ring3|reclaim|tale|spawn|splanc|ingle|leaf|<image> <want>..." >&2
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
