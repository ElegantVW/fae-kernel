//! Tiny 64-bit IDT so faults speak instead of a silent CPU reset.

use core::arch::asm;
use core::mem::size_of;

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

core::arch::global_asm!(
    ".global trap_stub",
    "trap_stub:",
    "cli",
    "lea rsi, [rip + trap_msg]",
    "1:",
    "lodsb",
    "test al, al",
    "jz 4f",
    "mov ah, al",
    "mov dx, 0x3FD",
    "2:",
    "in al, dx",
    "test al, 0x20",
    "jz 2b",
    "mov al, ah",
    "mov dx, 0x3F8",
    "out dx, al",
    "jmp 1b",
    "4:",
    "hlt",
    "jmp 4b",
    "trap_msg:",
    ".asciz \"kindling: trap\\n\"",
);

unsafe extern "C" {
    fn trap_stub();
}

fn gate(off: u64) -> IdtEntry {
    IdtEntry {
        off_lo: off as u16,
        selector: 0x18,
        ist: 0,
        flags: 0x8E,
        off_mid: (off >> 16) as u16,
        off_hi: (off >> 32) as u32,
        zero: 0,
    }
}

pub fn install() {
    let off = trap_stub as *const () as u64;
    unsafe {
        let idt = core::ptr::addr_of_mut!(IDT);
        for i in 0..256 {
            (*idt)[i] = gate(off);
        }
        let ptr = IdtPtr {
            limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u64,
        };
        asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
    }
}
