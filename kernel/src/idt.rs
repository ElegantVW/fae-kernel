//! 64-bit IDT. Vectors 0–31 name themselves. Selector is Kindling CS 0x08.
//! Vector 0xE0 is the house gate (`int 0xE0`, DPL 3) — see docs/HOUSECALLS.md.

use core::arch::asm;
use core::mem::size_of;

use crate::start::{hcf, serial_print, serial_u64};

/// House-call vector. User may knock (`DPL 3`).
pub const HOUSE_VEC: usize = 0xE0;
/// PIT IRQ0 vector (BIOS path).
pub const TIMER_VEC: usize = 0x20;
/// PS/2 keyboard IRQ1 vector (BIOS path).
pub const KBD_VEC: usize = 0x21;

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct IdtEntry {
    off_lo: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    off_mid: u16,
    off_hi: u32,
    zero: u32,
}

#[repr(C, packed)]
struct IdtPtr {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry {
    off_lo: 0,
    selector: 0,
    ist: 0,
    flags: 0,
    off_mid: 0,
    off_hi: 0,
    zero: 0,
}; 256];

macro_rules! vecs {
    (noerr: $($n:literal),*; err: $($e:literal),*) => {
        core::arch::global_asm!(
            $(
                concat!(
                    ".global vec_", stringify!($n), "\n",
                    "vec_", stringify!($n), ":\n",
                    "cli\n",
                    "push 0\n",
                    "mov rdi, ", stringify!($n), "\n",
                    "jmp trap_common\n",
                ),
            )*
            $(
                concat!(
                    ".global vec_", stringify!($e), "\n",
                    "vec_", stringify!($e), ":\n",
                    "cli\n",
                    "mov rdi, ", stringify!($e), "\n",
                    "jmp trap_common\n",
                ),
            )*
            // Frame at trap_common: [err][rip][cs][rflags][rsp][ss].
            // rsi = CS.RPL so guest (3) can smoor; ring-0 stays fatal.
            ".global trap_common",
            "trap_common:",
            "mov rsi, [rsp + 16]",
            "and rsi, 3",
            "and rsp, -16",
            "call trap_named",
            "hlt",
            "jmp trap_common",
            // House gate: `int 0xE0`. Entry: rax=n rdi=a0 rsi=a1 rdx=a2.
            // Callee-saved first so spawn can copy the Light (the coal)
            // before the child reuses RSP0. Shuffle to SysV, then
            // house_after(frame=rsp, ret=rax) kindles or smoors; else iretq.
            // Frame at house_after (rsp), qword slots:
            //   0 r11, 1 r10, 2 r9, 3 r8, 4 rcx, 5 rdx, 6 rsi, 7 rdi,
            //   8 r15, 9 r14, 10 r13, 11 r12, 12 rbp, 13 rbx,
            //   14 rip, 15 cs, 16 rflags, 17 rsp, 18 ss.
            ".global vec_house\n",
            "vec_house:\n",
            "push rbx\n",
            "push rbp\n",
            "push r12\n",
            "push r13\n",
            "push r14\n",
            "push r15\n",
            "push rdi\n",
            "push rsi\n",
            "push rdx\n",
            "push rcx\n",
            "push r8\n",
            "push r9\n",
            "push r10\n",
            "push r11\n",
            "mov rcx, rdx\n",
            "mov rdx, rsi\n",
            "mov rsi, rdi\n",
            "mov rdi, rax\n",
            "call house_entry\n",
            "mov rsi, rax\n",
            "mov rdi, rsp\n",
            "call house_after\n",
            "pop r11\n",
            "pop r10\n",
            "pop r9\n",
            "pop r8\n",
            "pop rcx\n",
            "pop rdx\n",
            "pop rsi\n",
            "pop rdi\n",
            "pop r15\n",
            "pop r14\n",
            "pop r13\n",
            "pop r12\n",
            "pop rbp\n",
            "pop rbx\n",
            "iretq\n",
            // PIT tick: IRQ0. Preserve everything, count, return.
            ".global vec_timer\n",
            "vec_timer:\n",
            "push rax\n",
            "push rcx\n",
            "push rdx\n",
            "push rsi\n",
            "push rdi\n",
            "push r8\n",
            "push r9\n",
            "push r10\n",
            "push r11\n",
            "call timer_tick\n",
            "pop r11\n",
            "pop r10\n",
            "pop r9\n",
            "pop r8\n",
            "pop rdi\n",
            "pop rsi\n",
            "pop rdx\n",
            "pop rcx\n",
            "pop rax\n",
            "iretq\n",
            // PS/2 keyboard: IRQ1. Preserve everything, drain, return.
            ".global vec_kbd\n",
            "vec_kbd:\n",
            "push rax\n",
            "push rcx\n",
            "push rdx\n",
            "push rsi\n",
            "push rdi\n",
            "push r8\n",
            "push r9\n",
            "push r10\n",
            "push r11\n",
            "call kbd_tick\n",
            "pop r11\n",
            "pop r10\n",
            "pop r9\n",
            "pop r8\n",
            "pop rdi\n",
            "pop rsi\n",
            "pop rdx\n",
            "pop rcx\n",
            "pop rax\n",
            "iretq\n",
        );
    };
}

vecs!(
    noerr: 0, 1, 2, 3, 4, 5, 6, 7, 9, 15, 16, 18, 19, 20, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31;
    err: 8, 10, 11, 12, 13, 14, 17, 21
);

unsafe extern "C" {
    fn vec_house();
    fn vec_timer();
    fn vec_kbd();
    fn vec_0();
    fn vec_1();
    fn vec_2();
    fn vec_3();
    fn vec_4();
    fn vec_5();
    fn vec_6();
    fn vec_7();
    fn vec_8();
    fn vec_9();
    fn vec_10();
    fn vec_11();
    fn vec_12();
    fn vec_13();
    fn vec_14();
    fn vec_15();
    fn vec_16();
    fn vec_17();
    fn vec_18();
    fn vec_19();
    fn vec_20();
    fn vec_21();
    fn vec_22();
    fn vec_23();
    fn vec_24();
    fn vec_25();
    fn vec_26();
    fn vec_27();
    fn vec_28();
    fn vec_29();
    fn vec_30();
    fn vec_31();
}

fn stubs() -> [*const (); 32] {
    [
        vec_0 as *const (),
        vec_1 as *const (),
        vec_2 as *const (),
        vec_3 as *const (),
        vec_4 as *const (),
        vec_5 as *const (),
        vec_6 as *const (),
        vec_7 as *const (),
        vec_8 as *const (),
        vec_9 as *const (),
        vec_10 as *const (),
        vec_11 as *const (),
        vec_12 as *const (),
        vec_13 as *const (),
        vec_14 as *const (),
        vec_15 as *const (),
        vec_16 as *const (),
        vec_17 as *const (),
        vec_18 as *const (),
        vec_19 as *const (),
        vec_20 as *const (),
        vec_21 as *const (),
        vec_22 as *const (),
        vec_23 as *const (),
        vec_24 as *const (),
        vec_25 as *const (),
        vec_26 as *const (),
        vec_27 as *const (),
        vec_28 as *const (),
        vec_29 as *const (),
        vec_30 as *const (),
        vec_31 as *const (),
    ]
}

fn current_cs() -> u16 {
    let mut cs: u16;
    unsafe {
        asm!("mov {:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
    }
    cs
}

fn gate(off: u64) -> IdtEntry {
    IdtEntry {
        off_lo: off as u16,
        selector: current_cs(),
        ist: 0,
        flags: 0x8E,
        off_mid: (off >> 16) as u16,
        off_hi: (off >> 32) as u32,
        zero: 0,
    }
}

/// House gate: present + DPL 3 interrupt gate so ring 3 may `int 0xE0`.
fn gate_user(off: u64) -> IdtEntry {
    IdtEntry {
        off_lo: off as u16,
        selector: current_cs(),
        ist: 0,
        flags: 0xEE,
        off_mid: (off >> 16) as u16,
        off_hi: (off >> 32) as u32,
        zero: 0,
    }
}

/// Same as [`gate`] but on IST `ist` (1–7). Double-fault rides IST1.
fn gate_ist(off: u64, ist: u8) -> IdtEntry {
    IdtEntry {
        off_lo: off as u16,
        selector: current_cs(),
        ist: ist & 7,
        flags: 0x8E,
        off_mid: (off >> 16) as u16,
        off_hi: (off >> 32) as u32,
        zero: 0,
    }
}

fn glass_trap(n: u64) {
    let mut b = [0u8; 32];
    let p = b"kindling: trap ";
    let mut i = 0usize;
    while i < p.len() {
        b[i] = p[i];
        i += 1;
    }
    if n >= 10 {
        b[i] = b'0' + ((n / 10) % 10) as u8;
        i += 1;
    }
    b[i] = b'0' + (n % 10) as u8;
    i += 1;
    b[i] = b'\n';
    i += 1;
    crate::glass::put_bytes(b.as_ptr(), i as u64);
}

/// `n` is the vector; `rpl` is CS.RPL (0 kernel, 3 guest).
/// Guest + a Light: drop the spark, spawn refuses. Guest alone: speak, halt.
/// Ring-0 and #DF stay fatal. Never resumes at the fault RIP.
#[unsafe(no_mangle)]
extern "sysv64" fn trap_named(n: u64, rpl: u64) -> ! {
    serial_print("kindling: trap ");
    serial_u64(n);
    serial_print("\n");
    if rpl == 3 && n != 8 {
        glass_trap(n);
        serial_print("the spark went out\n");
        let msg = b"the spark went out\n";
        crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
        crate::house::guest_went_out();
    }
    hcf();
}

pub fn install() {
    let s = stubs();
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
        let idt = core::ptr::addr_of_mut!(IDT);
        for i in 0..256 {
            let off = s[i.min(31)] as u64;
            (*idt)[i] = gate(off);
        }
        // The house gate earns its own stub — no aliasing to a trap.
        (*idt)[HOUSE_VEC] = gate_user(vec_house as *const () as u64);
        // PIT tick earns its own stub too (else IRQ0 wears trap 0's name).
        (*idt)[TIMER_VEC] = gate(vec_timer as *const () as u64);
        (*idt)[KBD_VEC] = gate(vec_kbd as *const () as u64);
        // Double-fault rides IST1 (gdt TSS) so a blown stack still speaks.
        (*idt)[8] = gate_ist(s[8] as u64, 1);
        let ptr = IdtPtr {
            limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u64,
        };
        asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
    }
}
