#![no_std]
#![no_main]

use core::arch::asm;
use core::fmt::{self, Write};

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

const COM1: u16 = 0x3F8;

#[inline]
unsafe fn outb(port: u16, val: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags)) }
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe {
        asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    val
}

fn serial_init() {
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

struct Serial;

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

fn serial_print(s: &str) {
    let _ = Serial.write_str(s);
}

/// Pink-ish fill so a windowed QEMU is not a black void.
fn paint_mark() {
    let Some(resp) = FRAMEBUFFER_REQUEST.get_response() else {
        return;
    };
    let Some(fb) = resp.framebuffers().next() else {
        return;
    };
    let w = fb.width().min(64);
    let h = fb.height().min(24);
    let pitch = fb.pitch();
    let bpp = fb.bpp();
    if bpp < 32 {
        return;
    }
    // #c44d7a-ish
    let pix: u32 = 0x00c4_4d7a;
    for y in 0..h {
        for x in 0..w {
            let off = (y * pitch + x * 4) as usize;
            unsafe {
                fb.addr().add(off).cast::<u32>().write_volatile(pix);
            }
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    let _ = &BASE_REVISION;
    serial_init();
    serial_print("fae-kernel\n");
    if !BASE_REVISION.is_supported() {
        serial_print("limine revision not supported\n");
        hcf();
    }
    paint_mark();
    serial_print("cerne: still only a spark\n");
    hcf();
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    serial_init();
    serial_print("panic: ");
    let _ = writeln!(Serial, "{info}");
    hcf();
}

fn hcf() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
