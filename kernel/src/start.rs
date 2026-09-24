use core::arch::asm;
use core::fmt::{self, Write};

use crate::mm::{self, Hint};

const COM1: u16 = 0x3F8;
// mana: firmware handed us a thimble at 0x7000. take_the_well pours a cup.

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
    // 16-color slot 13 Lilac, in case firmware did not already cast it.
    serial_print("\x1b[95m");
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

pub(crate) fn serial_u64(n: u64) {
    if n == 0 {
        serial_print("0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 20;
    let mut x = n;
    while x > 0 {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    let _ = Serial.write_str(core::str::from_utf8(&buf[i..]).unwrap());
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

/// Ceremony first. Then, if `hint` is Some, Kindling takes the well and a cup-stack.
pub fn start(hint: Option<Hint>) -> ! {
    serial_init();
    serial_print("fae-kernel\n");
    serial_print("kindling: still only a spark\n");
    if matches!(&hint, Some(h) if !h.trust_map) {
        crate::gdt::install();
    }
    crate::idt::install();
    if let Some(hint) = hint {
        if mm::five_level() {
            serial_print("kindling: well waits (5-level)\n");
            hcf();
        }
        if hint.trust_map {
            serial_print("kindling: efi map ");
            serial_u64(hint.ram_end / (1024 * 1024));
            serial_print(" MiB\n");
        }
        let well = unsafe { mm::prepare_well(hint) };
        unsafe {
            core::ptr::addr_of_mut!(WELL_MIB).write(mm::well_mib(well.ram_end));
            core::ptr::addr_of_mut!(CUP_KIB).write(mm::cup_kib(well.stack));
            core::ptr::addr_of_mut!(CANARY_AT).write(well.canary_at);
            asm!(
                "mov rsp, {top}",
                "mov cr3, {cr3}",
                "jmp {cont}",
                top = in(reg) well.top,
                cr3 = in(reg) well.cr3,
                cont = sym after_cup,
                options(noreturn)
            );
        }
    }
    hcf();
}

static mut WELL_MIB: u64 = 0;
static mut CUP_KIB: u64 = 0;
static mut CANARY_AT: u64 = 0;

#[inline(never)]
unsafe extern "C" fn after_cup() -> ! {
    if !mm::canary_ok(unsafe { core::ptr::addr_of!(CANARY_AT).read() }) {
        serial_print("kindling: the cup was bitten\n");
        hcf();
    }
    serial_print("kindling: well ");
    serial_u64(unsafe { core::ptr::addr_of!(WELL_MIB).read() });
    serial_print(" MiB · stack ");
    serial_u64(unsafe { core::ptr::addr_of!(CUP_KIB).read() });
    serial_print(" KiB cup\n");
    hcf();
}

pub fn hcf() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
