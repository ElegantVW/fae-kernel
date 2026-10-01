//! House calls — Gleam's own gate (`int 0xE0`). Not Linux.
//! Numbers survive the future `syscall/sysret` upgrade.

#[cfg(feature = "house-test")]
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
        crate::cairn::GLEAN => crate::cairn::glean(a0, a1, a2),
        SPAWN | GRANT | FLUSH | crate::cairn::STOW => err(EAGAIN),
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
    // Timer alive? Latch PIT ch0 twice around a spin; a running counter
    // must differ. A dead PIT would hang sleep forever — fail here instead.
    unsafe {
        pout(0x43, 0x00);
        let c1 = pin(0x40) as u16 | ((pin(0x40) as u16) << 8);
        let mut spin = 0u32;
        while spin < 300_000 {
            core::hint::spin_loop();
            spin += 1;
        }
        pout(0x43, 0x00);
        let c2 = pin(0x40) as u16 | ((pin(0x40) as u16) << 8);
        if c1 == c2 {
            fail = 7;
        }
        if pin(0x21) & 1 != 0 {
            fail = 8; // IRQ0 still masked — timer::init did not hold.
        }
        // Virtual wire proof: SVR enabled, LINT0 ExtINT-unmasked, base sane.
        if crate::timer::svr_seen() & 0x100 == 0 {
            fail = 9;
        }
        let lint = crate::timer::lint_seen();
        if lint & 0x10000 != 0 || lint & 0x700 != 0x700 {
            fail = 10;
        }
        let base = crate::timer::apic_base_seen();
        if base & 0xFFFF_F000 != 0xFEE0_0000 {
            fail = 11;
        }
        if base & (1 << 11) == 0 {
            fail = 12; // LAPIC disabled at the MSR — MMIO was a whisper.
        }
    }
    if fail != 0 {
        serial_print("kindling: house FAIL ");
        serial_u64(fail);
        serial_print("\n");
        hcf();
    }
    serial_print("kindling: house ok\n");
}

#[cfg(feature = "house-test")]
unsafe fn pout(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags))
    }
}

#[cfg(feature = "house-test")]
unsafe fn pin(port: u16) -> u8 {
    let v: u8;
    unsafe {
        asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    v
}

// --- CPL3 entry: inits and sparks alike ride this iretq. ---
// RSP0 must already point at the kernel cup (caller sets it), so CPL3
// interrupts land home. Never returns.
#[cfg(any(feature = "ring3-test", feature = "tale-test"))]
static mut USTACK: [u8; 16384] = [0; 16384];

#[cfg(any(feature = "ring3-test", feature = "tale-test"))]
pub fn enter_user(entry: u64) -> ! {
    let ustack_top = (core::ptr::addr_of!(USTACK) as u64 + 16384) & !0xF;
    unsafe {
        // NOTE: addresses ride fixed regs — never `in(reg)` (it may pick
        // RSP), never `push {sym}` (that pushes the qword AT the symbol;
        // there is no push-imm64).
        core::arch::asm!(
            "mov ax, 0x23",
            "mov ds, ax",
            "mov es, ax",
            "xor eax, eax",
            "mov fs, ax",
            "mov gs, ax",
            "push {ss}",
            "push r10",
            "push {rflags}",
            "push {cs}",
            "push r11",
            "iretq",
            ss = const crate::gdt::UDATA_RPL3,
            rflags = const 0x202u64,
            cs = const crate::gdt::UCODE_RPL3,
            in("r10") ustack_top,
            in("r11") entry,
            options(noreturn),
        );
    }
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
    // Sleep 50 ms, then prove a tick passed (timer IRQ + RSP0 from CPL3).
    "mov rax, 4",
    "int 0xE0",
    "mov r12, rax",
    "mov rax, 3",
    "mov rdi, 50",
    "int 0xE0",
    "mov rax, 4",
    "int 0xE0",
    "cmp rax, r12",
    "ja 5f",
    "mov rax, 2",
    "mov rdi, 1",
    "lea rsi, [rip + 6f]",
    "mov edx, 18",
    "int 0xE0",
    "mov rax, 1",
    "mov rdi, 1",
    "int 0xE0",
    "ud2",
    "5:",
    "mov rax, 2",
    "mov rdi, 1",
    "lea rsi, [rip + 8f]",
    "mov edx, 21",
    "int 0xE0",
    "xor eax, eax",
    "int 0xE0",
    "mov rax, 1",
    "xor edi, edi",
    "int 0xE0",
    "ud2",
    "2:",
    ".ascii \"kindling: init ok\\n\"",
    "6:",
    ".ascii \"kindling: no tick\\n\"",
    "8:",
    ".ascii \"kindling: init slept\\n\"",
);

#[cfg(feature = "ring3-test")]
unsafe extern "C" {
    fn gleam_init();
}

/// Drop to CPL3 at `gleam_init` on the private stack. Never returns.
#[cfg(feature = "ring3-test")]
pub fn enter_init(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    enter_user(gleam_init as *const () as u64);
}
