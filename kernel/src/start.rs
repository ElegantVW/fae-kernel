use core::arch::asm;
use core::fmt::{self, Write};

const COM1: u16 = 0x3F8;
// mana: this early stack is a thimble (B). the well (GiB) comes in phase 1.

#[inline]
unsafe fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags))
    }
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe {
        asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    val
}

pub fn serial_init() {
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1 + 0, 0x03);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xC7);
        outb(COM1 + 4, 0x0B);
    }
}

fn serial_put(b: u8) {
    unsafe {
        while inb(COM1 + 5) & 0x20 == 0 {
            core::hint::spin_loop();
        }
        outb(COM1, b);
    }
}

pub struct Serial;

impl Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                serial_put(b'\r');
            }
            serial_put(b);
        }
        Ok(())
    }
}

pub fn serial_print(s: &str) {
    let _ = Serial.write_str(s);
}

/// Optional GOP/Limine framebuffer. Null addr skips the mark.
#[allow(dead_code)]
pub fn paint_mark(addr: *mut u8, width: u64, height: u64, pitch: u64, bpp: u16) {
    if addr.is_null() || bpp < 32 {
        return;
    }
    let w = width.min(64);
    let h = height.min(24);
    let pix: u32 = 0x00c4_4d7a;
    for y in 0..h {
        for x in 0..w {
            let off = (y * pitch + x * 4) as usize;
            unsafe {
                addr.add(off).cast::<u32>().write_volatile(pix);
            }
        }
    }
}

pub fn start() -> ! {
    serial_init();
    serial_print("fae-kernel\n");
    serial_print("kindling: still only a spark\n");
    hcf();
}

pub fn hcf() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
