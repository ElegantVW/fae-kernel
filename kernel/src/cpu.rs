//! FPU/SSE and 8259 PIC — so the spark does not #UD or take a stray IRQ.

use core::arch::asm;

/// Remap both 8259s to 0x20/0x28 (8086 mode) and mask every line.
/// Firmware already did this on the BIOS path; EFI/OVMF often leaves the
/// PIC on the default 0x08 vectors, which collide with exceptions.
pub fn init_pic() {
    unsafe {
        asm!("out 0x20, al", in("al") 0x11u8, options(nostack, nomem));
        asm!("out 0xA0, al", in("al") 0x11u8, options(nostack, nomem));
        asm!("out 0x21, al", in("al") 0x20u8, options(nostack, nomem));
        asm!("out 0xA1, al", in("al") 0x28u8, options(nostack, nomem));
        asm!("out 0x21, al", in("al") 0x04u8, options(nostack, nomem));
        asm!("out 0xA1, al", in("al") 0x02u8, options(nostack, nomem));
        asm!("out 0x21, al", in("al") 0x01u8, options(nostack, nomem));
        asm!("out 0xA1, al", in("al") 0x01u8, options(nostack, nomem));
    }
    mask_pic();
}

pub fn mask_pic() {
    unsafe {
        asm!("out 0x21, al", in("al") 0xFFu8, options(nostack, nomem));
        asm!("out 0xA1, al", in("al") 0xFFu8, options(nostack, nomem));
    }
}

/// CR0.EM=0 MP=1; CR4 OSFXSR+OSXMMEXCPT; fninit.
pub fn enable_fpu_sse() {
    unsafe {
        asm!(
            "mov {0}, cr0",
            "and {0}, {em}",
            "or {0}, {mp}",
            "mov cr0, {0}",
            "mov {0}, cr4",
            "or {0}, {sse}",
            "mov cr4, {0}",
            "clts",
            "fninit",
            out(reg) _,
            em = const !(1u64 << 2),
            mp = const (1u64 << 1),
            sse = const (1u64 << 9) | (1u64 << 10),
        );
        #[repr(align(16))]
        struct X([u8; 16]);
        static ZERO: X = X([0; 16]);
        let _ = ZERO.0[0];
        asm!(
            "movaps xmm0, [{z}]",
            z = in(reg) core::ptr::addr_of!(ZERO),
            options(nostack),
        );
    }
}
