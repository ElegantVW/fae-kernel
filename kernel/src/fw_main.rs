//! Identity-mapped kernel for cerne-fw (`jmp 0x200000`).
#![no_std]
#![no_main]

mod mm;
mod start;

use core::fmt::Write;
use mm::Hint;
use start::{serial_print, start, Serial};

unsafe extern "C" {
    static __kernel_end: u8;
}

core::arch::global_asm!(
    ".section .text.magic,\"ax\",@progbits",
    ".global kindle_vector",
    "kindle_vector:",
    ".ascii \"KNDL\"",
    "jmp kmain",
);

const FMAP: u32 = 0x5041_4D46; // 'FMAP'

#[unsafe(link_section = ".text.entry")]
#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    let kernel_end = unsafe { &__kernel_end as *const u8 as u64 };
    let ram_end = unsafe {
        let magic = core::ptr::read_volatile(0x8000 as *const u32);
        if magic == FMAP {
            core::ptr::read_volatile(0x8008 as *const u32) as u64
        } else {
            0
        }
    };
    start(Some(Hint {
        kernel_end,
        ram_end,
    }))
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    start::serial_init();
    serial_print("the spark went out: ");
    let _ = writeln!(Serial, "{info}");
    start::hcf();
}
