//! 64-bit IDT. Vectors 0–31 name themselves. Selector is Kindling CS 0x08.

use core::arch::asm;
use core::mem::size_of;

use crate::start::{hcf, serial_print, serial_u64};

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
    ($($n:literal),*) => {
        core::arch::global_asm!(
            $(
                concat!(
                    ".global vec_", stringify!($n), "\n",
                    "vec_", stringify!($n), ":\n",
                    "cli\n",
                    "mov rdi, ", stringify!($n), "\n",
                    "jmp trap_common\n",
                ),
            )*
            ".global trap_common",
            "trap_common:",
            "and rsp, -16",
            "call trap_named",
            "hlt",
            "jmp trap_common",
        );
    };
}

vecs!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31
);

unsafe extern "C" {
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

#[unsafe(no_mangle)]
extern "C" fn trap_named(n: u64) -> ! {
    serial_print("kindling: trap ");
    serial_u64(n);
    serial_print("\n");
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
        let ptr = IdtPtr {
            limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u64,
        };
        asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
    }
}
