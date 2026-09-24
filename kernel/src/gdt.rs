//! Kindling's own GDT. After this, CS is 0x08 — not firmware 0x18.

use core::arch::asm;

#[repr(C, packed)]
struct GdtPtr {
    limit: u16,
    base: u64,
}

static mut GDT: [u64; 3] = [
    0,
    0x00AF_9A00_0000_FFFF, // 0x08 64-bit code
    0x00AF_9200_0000_FFFF, // 0x10 64-bit data
];

#[allow(dead_code)]
pub const KCODE: u16 = 0x08;
#[allow(dead_code)]
pub const KDATA: u16 = 0x10;

pub fn install() {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
        let ptr = GdtPtr {
            limit: 23,
            base: core::ptr::addr_of!(GDT) as u64,
        };
        asm!("lgdt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
        asm!(
            "push {cs}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",
            cs = const 8u64,
            tmp = out(reg) _,
        );
        asm!(
            "mov {0:x}, {d}",
            "mov ds, {0:x}",
            "mov es, {0:x}",
            "mov ss, {0:x}",
            "mov fs, {0:x}",
            "mov gs, {0:x}",
            out(reg) _,
            d = const 0x10u16,
        );
    }
}
