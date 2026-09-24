//! FPU/SSE and 8259 PIC — so the spark does not #UD or take a stray IRQ.

use core::arch::asm;

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
