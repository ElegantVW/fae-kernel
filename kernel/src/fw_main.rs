//! Identity-mapped kernel for cerne-fw (`jmp 0x200000`).
#![no_std]
#![no_main]

mod ata;
mod cairn;
mod cpu;
mod gdt;
mod house;
mod idt;
mod mm;
mod start;
mod timer;

use core::fmt::Write;
use mm::Hint;
use start::{Serial, serial_print, start};

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
const FMAP_KEY: u32 = 0x4C44_4E4B; // 'KNDL'

fn fmap_ram_end() -> u64 {
    unsafe {
        let magic = core::ptr::read_volatile(0x8000 as *const u32);
        let sum = core::ptr::read_volatile(0x8004 as *const u32);
        let ram = core::ptr::read_volatile(0x8008 as *const u32);
        let nreg = core::ptr::read_volatile(0x800C as *const u32);
        if magic == FMAP && sum == (ram ^ nreg ^ FMAP_KEY) {
            ram as u64
        } else {
            0
        }
    }
}

#[unsafe(link_section = ".text.entry")]
#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    let kernel_end = unsafe { &__kernel_end as *const u8 as u64 };
    let ram_end = fmap_ram_end();
    start(Some(Hint {
        kernel_end,
        ram_end,
        trust_map: false,
    }))
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    start::serial_init();
    serial_print("the spark went out: ");
    let _ = writeln!(Serial, "{info}");
    start::hcf();
}
