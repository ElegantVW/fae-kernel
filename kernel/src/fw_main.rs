//! Identity-mapped kernel for cerne-fw (`jmp 0x200000`).
#![no_std]
#![no_main]

mod start;

use core::fmt::Write;
use start::{serial_print, start, Serial};

#[unsafe(link_section = ".text.entry")]
#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    start()
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    start::serial_init();
    serial_print("panic: ");
    let _ = writeln!(Serial, "{info}");
    start::hcf();
}
