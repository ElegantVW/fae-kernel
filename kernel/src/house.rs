//! House calls — Gleam's own gate (`int 0xE0`). Not Linux.
//! Numbers survive the future `syscall/sysret` upgrade.

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
            crate::timer::sleep_ms(a0);
            0
        }
        TIME => crate::timer::ms(),
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
    use core::arch::asm;
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

// --- Ring-3 init: the first Gleam light. Speaks only via `int 0xE0`. ---
#[cfg(feature = "ring3-test")]
core::arch::global_asm!(
    ".global gleam_init",
    "gleam_init:",
    "mov rax, 2",
    "mov rdi, 1",
    "lea rsi, [rip + 2f]",
    "mov edx, 18",
    "int 0xE0",
    "xor eax, eax",
    "int 0xE0",
    "mov rax, 1",
    "xor edi, edi",
    "int 0xE0",
    "ud2",
    "2:",
    ".ascii \"kindling: init ok\\n\"",
    "4:",
);

#[cfg(feature = "ring3-test")]
static mut USTACK: [u8; 16384] = [0; 16384];

#[cfg(feature = "ring3-test")]
unsafe extern "C" {
    fn gleam_init();
}

/// Drop to CPL3 at `gleam_init` on a private stack. RSP0 already points
/// at the kernel cup, so CPL3 interrupts land home. Never returns.
#[cfg(feature = "ring3-test")]
pub fn enter_init(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let ustack_top = (core::ptr::addr_of!(USTACK) as u64 + 16384) & !0xF;
    unsafe {
        // NOTE: ustack_top rides in R10 and the entry in R11 — never
        // `in(reg)` (it may pick RSP), and never `push {sym}` (that pushes
        // the qword AT the symbol; there is no push-imm64).
        core::arch::asm!(
            "mov ax, 0x23",
            "mov ds, ax",
            "mov es, ax",
            "xor eax, eax",
            "mov fs, ax",
            "mov gs, ax",
            "lea r11, [rip + {init}]",
            "push {ss}",
            "push r10",
            "push {rflags}",
            "push {cs}",
            "push r11",
            "iretq",
            ss = const crate::gdt::UDATA_RPL3,
            rflags = const 0x202u64,
            cs = const crate::gdt::UCODE_RPL3,
            init = sym gleam_init,
            in("r10") ustack_top,
            options(noreturn),
        );
    }
}
