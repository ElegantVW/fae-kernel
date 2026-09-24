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

#[unsafe(link_section = ".text.entry")]
#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    let kernel_end = unsafe { &__kernel_end as *const u8 as u64 };
    start(Some(Hint {
        kernel_end,
        ram_end: 256 * 1024 * 1024,
    }))
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    start::serial_init();
    serial_print("the spark went out: ");
    let _ = writeln!(Serial, "{info}");
    start::hcf();
}
