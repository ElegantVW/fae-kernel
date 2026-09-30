//! House calls — Gleam's own gate (`int 0xE0`). Not Linux.
//! Numbers survive the future `syscall/sysret` upgrade.

use core::arch::asm;

use crate::start::{hcf, serial_print, serial_u64};

pub const YIELD: u64 = 0;
pub const EXIT: u64 = 1;
pub const WRITE: u64 = 2;
pub const SLEEP: u64 = 3;
pub const TIME: u64 = 4;
pub const SPAWN: u64 = 5;
pub const GRANT: u64 = 6;
pub const FLUSH: u64 = 7;

const EPERM: u64 = 1;
const EAGAIN: u64 = 11;
const ENOSYS: u64 = 38;

fn err(n: u64) -> u64 {
    0u64.wrapping_sub(n)
}

fn write_serial(buf: *const u8, len: u64) -> u64 {
    if buf.is_null() || len > (1 << 20) {
        return err(EPERM);
    }
    let mut done = 0u64;
    let mut i = 0u64;
    while i < len {
        let b = unsafe { buf.add(i as usize).read_volatile() };
        // Serial honors `\n` -> `\r\n` via the shared putter path.
        crate::start::serial_put_byte(b);
        done += 1;
        i += 1;
    }
    done
}

fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// Kernel dispatcher. Pure + testable: no asm here, `int 0xE0` stub calls
/// [`house_entry`] which forwards here.
pub fn dispatch(n: u64, a0: u64, a1: u64, a2: u64) -> u64 {
    match n {
        YIELD => 0,
        EXIT => {
            serial_print("kindling: gleam exit ");
            serial_u64(a0);
            serial_print("\n");
            hcf();
        }
        WRITE => {
            if a0 == 1 || a0 == 2 {
                write_serial(a1 as *const u8, a2)
            } else {
                err(EPERM)
            }
        }
        SLEEP => {
            let _ = a0;
            0
        }
        TIME => rdtsc(),
        SPAWN | GRANT | FLUSH => err(EAGAIN),
        _ => err(ENOSYS),
    }
}

/// C entry for the asm stub: `(n, a0, a1, a2) -> ret`.
#[unsafe(no_mangle)]
pub extern "C" fn house_entry(n: u64, a0: u64, a1: u64, a2: u64) -> u64 {
    dispatch(n, a0, a1, a2)
}

/// Ring-0 self-test: direct dispatch + a real `int 0xE0` knock.
/// Prints `kindling: house ok` or a `kindling: house FAIL ...` line.
/// On success returns; on failure halts (never returns).
#[cfg(feature = "house-test")]
pub fn self_test() {
    // Direct: yield ok, unknown refuses, wrong fd refuses, shut calls wait.
    let mut fail = 0u64;
    if dispatch(YIELD, 0, 0, 0) != 0 {
        fail = 1;
    }
    if dispatch(0xC0FFEE, 0, 0, 0) != err(ENOSYS) {
        fail = 2;
    }
    if dispatch(WRITE, 7, 0, 0) != err(EPERM) {
        fail = 3;
    }
    if dispatch(SPAWN, 0, 0, 0) != err(EAGAIN) {
        fail = 4;
    }
    // Real gate: `int 0xE0` yield must return 0 in rax.
    let ret: u64;
    unsafe {
        asm!(
            "mov rax, {n}",
            "xor edi, edi",
            "xor esi, esi",
            "xor edx, edx",
            "int 0xE0",
            "mov {out}, rax",
            n = const YIELD,
            out = out(reg) ret,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        );
    }
    if ret != 0 {
        fail = 5;
    }
    // Real gate: `int 0xE0` write "ok\n" to fd 1 must return 3.
    let msg = b"ok\n";
    let w: u64;
    unsafe {
        asm!(
            "mov rax, {n}",
            "mov rdi, 1",
            "mov rsi, {buf}",
            "mov rdx, 3",
            "int 0xE0",
            "mov {out}, rax",
            n = const WRITE,
            buf = in(reg) msg.as_ptr(),
            out = out(reg) w,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        );
    }
    if w != 3 {
        fail = 6;
    }
    if fail != 0 {
        serial_print("kindling: house FAIL ");
        serial_u64(fail);
        serial_print("\n");
        hcf();
    }
    serial_print("kindling: house ok\n");
}
