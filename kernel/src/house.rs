//! House calls — Gleam's own gate (`int 0xE0`). Not Linux.
//! Numbers survive the future `syscall/sysret` upgrade.

use core::arch::asm;
use core::ptr::{addr_of, addr_of_mut};

use crate::mm::SparkRealm;
use crate::start::{hcf, serial_print, serial_u64};

pub const YIELD: u64 = 0;
pub const EXIT: u64 = 1;
pub const WRITE: u64 = 2;
pub const SLEEP: u64 = 3;
pub const TIME: u64 = 4;
pub const SPAWN: u64 = 5;
pub const GRANT: u64 = 6;
pub const FLUSH: u64 = 7;
pub const READ: u64 = 10;

const EPERM: u64 = 1;
#[cfg(feature = "house-test")]
const ENOENT: u64 = 2;
const EAGAIN: u64 = 11;
#[cfg(feature = "house-test")]
const ENODEV: u64 = 19;
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
            if light_live() {
                let realm = take_child();
                crate::mm::drop_spark(&realm);
                smoor(a0);
                0
            } else {
                serial_print("kindling: gleam exit ");
                serial_u64(a0);
                serial_print("\n");
                hcf();
            }
        }
        WRITE => {
            if a0 == 1 || a0 == 2 {
                let n = write_serial(a1 as *const u8, a2);
                if (n as i64) >= 0 {
                    crate::glass::put_bytes(a1 as *const u8, a2);
                }
                n
            } else {
                err(EPERM)
            }
        }
        READ => {
            if a0 == 0 {
                crate::kbd::read(a1 as *mut u8, a2)
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
        crate::cairn::STOW => crate::cairn::stow(a0, a1, a2),
        SPAWN => crate::cairn::spawn(a0),
        GRANT | FLUSH => err(EAGAIN),
        _ => err(ENOSYS),
    }
}

/// C entry for the asm stub: `(n, a0, a1, a2) -> ret`.
/// Asm stub shuffles into SysV (`rdi,rsi,rdx,rcx`). The UEFI target's
/// `extern "C"` is win64, so this is pinned.
#[unsafe(no_mangle)]
pub extern "sysv64" fn house_entry(n: u64, a0: u64, a1: u64, a2: u64) -> u64 {
    dispatch(n, a0, a1, a2)
}

/// After `house_entry`: kindle a waiting spark, smoor into the Light, or
/// hand `ret` back so `vec_house` can iretq. `frame` is rsp at the 19-qword
/// house frame (see `idt.rs`).
#[unsafe(no_mangle)]
pub extern "sysv64" fn house_after(frame: u64, ret: u64) -> u64 {
    let sw = unsafe { addr_of!(SWITCH).read_volatile() };
    match sw {
        1 => {
            save_light(frame);
            unsafe { addr_of_mut!(SWITCH).write_volatile(0) };
            house_kindle_enter();
        }
        2 => {
            unsafe { addr_of_mut!(SWITCH).write_volatile(0) };
            house_smoor_enter();
        }
        _ => ret,
    }
}

// --- G30: one Light. Kindle a spark; when it smoors, spawn returns. ---

const SWITCH_KINDLE: u64 = 1;
const SWITCH_SMOOR: u64 = 2;

// House frame slots (qword), matching vec_house after `call house_entry`.
const FR_R11: usize = 0;
const FR_R10: usize = 1;
const FR_R9: usize = 2;
const FR_R8: usize = 3;
const FR_RCX: usize = 4;
const FR_RDX: usize = 5;
const FR_RSI: usize = 6;
const FR_RDI: usize = 7;
const FR_R15: usize = 8;
const FR_R14: usize = 9;
const FR_R13: usize = 10;
const FR_R12: usize = 11;
const FR_RBP: usize = 12;
const FR_RBX: usize = 13;
const FR_RIP: usize = 14;
const FR_CS: usize = 15;
const FR_RFLAGS: usize = 16;
const FR_RSP: usize = 17;
const FR_SS: usize = 18;

#[repr(C)]
struct Light {
    live: u64,
    word: u64,
    cr3: u64,
    rbx: u64,
    rbp: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
    child: SparkRealm,
}

static mut LIGHT: Light = Light {
    live: 0,
    word: 0,
    cr3: 0,
    rbx: 0,
    rbp: 0,
    r12: 0,
    r13: 0,
    r14: 0,
    r15: 0,
    r11: 0,
    r10: 0,
    r9: 0,
    r8: 0,
    rcx: 0,
    rdx: 0,
    rsi: 0,
    rdi: 0,
    rip: 0,
    cs: 0,
    rflags: 0,
    rsp: 0,
    ss: 0,
    child: SparkRealm {
        cr3: 0,
        spark: 0,
        cup_top: 0,
        spark_len: 0,
        cup_base: 0,
        guard: 0,
    },
};

static mut SWITCH: u64 = 0;

pub fn light_live() -> bool {
    unsafe { addr_of!(LIGHT).read_volatile().live != 0 }
}

/// Kindling will iretq into `realm` after this house call returns.
pub fn kindle(realm: SparkRealm) {
    unsafe {
        let l = addr_of_mut!(LIGHT);
        (*l).child = realm;
        (*l).live = 1;
        addr_of_mut!(SWITCH).write_volatile(SWITCH_KINDLE);
    }
}

fn take_child() -> SparkRealm {
    unsafe { addr_of!(LIGHT).read_volatile().child }
}

/// The Light's spawn receives `word` in rax.
pub fn smoor(word: u64) {
    unsafe {
        let l = addr_of_mut!(LIGHT);
        (*l).word = word;
        (*l).live = 0;
        addr_of_mut!(SWITCH).write_volatile(SWITCH_SMOOR);
    }
}

fn save_light(frame: u64) {
    unsafe {
        let f = frame as *const u64;
        let l = addr_of_mut!(LIGHT);
        (*l).r11 = f.add(FR_R11).read();
        (*l).r10 = f.add(FR_R10).read();
        (*l).r9 = f.add(FR_R9).read();
        (*l).r8 = f.add(FR_R8).read();
        (*l).rcx = f.add(FR_RCX).read();
        (*l).rdx = f.add(FR_RDX).read();
        (*l).rsi = f.add(FR_RSI).read();
        (*l).rdi = f.add(FR_RDI).read();
        (*l).r15 = f.add(FR_R15).read();
        (*l).r14 = f.add(FR_R14).read();
        (*l).r13 = f.add(FR_R13).read();
        (*l).r12 = f.add(FR_R12).read();
        (*l).rbp = f.add(FR_RBP).read();
        (*l).rbx = f.add(FR_RBX).read();
        (*l).rip = f.add(FR_RIP).read();
        (*l).cs = f.add(FR_CS).read();
        (*l).rflags = f.add(FR_RFLAGS).read();
        (*l).rsp = f.add(FR_RSP).read();
        (*l).ss = f.add(FR_SS).read();
        let cr3: u64;
        asm!("mov {}, cr3", out(reg) cr3, options(nostack, preserves_flags));
        (*l).cr3 = cr3;
    }
}

fn house_kindle_enter() -> ! {
    let r = unsafe { addr_of!(LIGHT).read_volatile().child };
    enter_user_in(r.spark, r.cup_top, r.cr3);
}

fn house_smoor_enter() -> ! {
    let p = addr_of!(LIGHT) as u64;
    unsafe {
        asm!(
            "mov r9, [r8 + {off_cr3}]",
            "mov cr3, r9",
            "mov ax, 0x23",
            "mov ds, ax",
            "mov es, ax",
            "mov rbx, [r8 + {off_rbx}]",
            "mov rbp, [r8 + {off_rbp}]",
            "mov r12, [r8 + {off_r12}]",
            "mov r13, [r8 + {off_r13}]",
            "mov r14, [r8 + {off_r14}]",
            "mov r15, [r8 + {off_r15}]",
            "mov r9, [r8 + {off_ss}]",
            "push r9",
            "mov r9, [r8 + {off_rsp}]",
            "push r9",
            "mov r9, [r8 + {off_rflags}]",
            "push r9",
            "mov r9, [r8 + {off_cs}]",
            "push r9",
            "mov r9, [r8 + {off_rip}]",
            "push r9",
            "mov rax, [r8 + {off_word}]",
            "mov r11, [r8 + {off_r11}]",
            "mov r10, [r8 + {off_r10}]",
            "mov r9, [r8 + {off_r9}]",
            "mov rcx, [r8 + {off_rcx}]",
            "mov rdx, [r8 + {off_rdx}]",
            "mov rsi, [r8 + {off_rsi}]",
            "mov rdi, [r8 + {off_rdi}]",
            "mov r8, [r8 + {off_r8}]",
            "iretq",
            in("r8") p,
            off_cr3 = const 16u64,
            off_word = const 8u64,
            off_rbx = const 24u64,
            off_rbp = const 32u64,
            off_r12 = const 40u64,
            off_r13 = const 48u64,
            off_r14 = const 56u64,
            off_r15 = const 64u64,
            off_r11 = const 72u64,
            off_r10 = const 80u64,
            off_r9 = const 88u64,
            off_r8 = const 96u64,
            off_rcx = const 104u64,
            off_rdx = const 112u64,
            off_rsi = const 120u64,
            off_rdi = const 128u64,
            off_rip = const 136u64,
            off_cs = const 144u64,
            off_rflags = const 152u64,
            off_rsp = const 160u64,
            off_ss = const 168u64,
            options(noreturn),
        );
    }
}

/// Ring-0 self-test: direct dispatch + a real `int 0xE0` knock.
/// Prints `kindling: house ok` or a `kindling: house FAIL ...` line.
/// On success returns; on failure halts (never returns).
#[cfg(feature = "house-test")]
pub fn self_test() {
    // Direct: yield ok, unknown refuses, wrong fd refuses, spawn null
    // refuses. A missing name is `-ENODEV` (no cairn) or `-ENOENT` (cairn
    // packed, no such spark) — never success, never `-EAGAIN`.
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
    if dispatch(READ, 1, 0, 0) != err(EPERM) {
        fail = 14;
    }
    if dispatch(SPAWN, 0, 0, 0) != err(EPERM) {
        fail = 4;
    }
    let ghost = b"x\0";
    let g = dispatch(SPAWN, ghost.as_ptr() as u64, 0, 0);
    if g != err(ENODEV) && g != err(ENOENT) {
        fail = 13;
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
//
// `enter_user_in` loads the realm CR3 then iretq. Tale/init keep
// `enter_user` on the shared map (static USTACK, live CR3).

/// Drop to CPL3 at `entry` with this user stack and CR3. Never returns.
pub fn enter_user_in(entry: u64, ustack_top: u64, cr3: u64) -> ! {
    unsafe {
        // NOTE: addresses ride fixed regs — never `in(reg)` (it may pick
        // RSP), never `push {sym}` (that pushes the qword AT the symbol;
        // there is no push-imm64).
        core::arch::asm!(
            "mov cr3, r9",
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
            in("r9") cr3,
            in("r10") ustack_top,
            in("r11") entry,
            options(noreturn),
        );
    }
}

#[cfg(any(feature = "ring3-test", feature = "tale-test"))]
static mut USTACK: [u8; 16384] = [0; 16384];

#[cfg(any(feature = "ring3-test", feature = "tale-test"))]
pub fn enter_user(entry: u64) -> ! {
    let ustack_top = (core::ptr::addr_of!(USTACK) as u64 + 16384) & !0xF;
    unsafe {
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
