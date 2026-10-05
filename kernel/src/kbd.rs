//! PS/2 8042 keyboard (BIOS path). IRQ1, scancode set 1 → ASCII.
//! Absent controller is `-EAGAIN`, never a hang.

use core::arch::asm;

const DATA: u16 = 0x60;
const STAT: u16 = 0x64;
const CAP: usize = 64;
const SPINS: u32 = 100_000;

const EPERM: u64 = 1;
const EAGAIN: u64 = 11;

static mut BUF: [u8; CAP] = [0; CAP];
static mut HEAD: usize = 0;
static mut TAIL: usize = 0;
static mut LIVE: bool = false;
static mut EXT: bool = false;

#[inline]
unsafe fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
    }
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe {
        asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    v
}

fn err(n: u64) -> u64 {
    0u64.wrapping_sub(n)
}

fn wait_out() -> bool {
    unsafe {
        let mut n = 0u32;
        while inb(STAT) & 2 != 0 {
            n = n.saturating_add(1);
            if n >= SPINS {
                return false;
            }
            core::hint::spin_loop();
        }
    }
    true
}

fn wait_in() -> bool {
    unsafe {
        let mut n = 0u32;
        while inb(STAT) & 1 == 0 {
            n = n.saturating_add(1);
            if n >= SPINS {
                return false;
            }
            core::hint::spin_loop();
        }
    }
    true
}

fn cmd(b: u8) -> bool {
    if !wait_out() {
        return false;
    }
    unsafe {
        outb(STAT, b);
    }
    true
}

fn data(b: u8) -> bool {
    if !wait_out() {
        return false;
    }
    unsafe {
        outb(DATA, b);
    }
    true
}

fn flush() {
    unsafe {
        let mut n = 0u32;
        while inb(STAT) & 1 != 0 {
            let _ = inb(DATA);
            n = n.saturating_add(1);
            if n >= 16 {
                break;
            }
        }
    }
}

/// Probe the 8042, enable IRQ1, unmask PIC IRQ1. Quiet if the port is dead.
pub fn init() {
    unsafe {
        if inb(STAT) == 0xFF {
            return;
        }
        if !cmd(0xAD) || !cmd(0xA7) {
            return;
        }
        flush();
        if !cmd(0x20) || !wait_in() {
            return;
        }
        let mut cfg = inb(DATA);
        cfg = (cfg | 0x01 | 0x40) & !0x10;
        if !cmd(0x60) || !data(cfg) || !cmd(0xAE) {
            return;
        }
        // Enable scanning. Keyboard ACK (0xFA) is drained by IRQ1 / flush.
        let _ = data(0xF4);
        let m = inb(0x21) & !0x02;
        outb(0x21, m);
        core::ptr::addr_of_mut!(LIVE).write(true);
    }
}

fn push(b: u8) {
    if b == 0 {
        return;
    }
    unsafe {
        let head = core::ptr::addr_of!(HEAD).read();
        let tail = core::ptr::addr_of!(TAIL).read();
        let next = (head + 1) % CAP;
        if next == tail {
            return;
        }
        core::ptr::addr_of_mut!(BUF).cast::<u8>().add(head).write(b);
        core::ptr::addr_of_mut!(HEAD).write(next);
    }
}

fn pop() -> Option<u8> {
    unsafe {
        let head = core::ptr::addr_of!(HEAD).read();
        let tail = core::ptr::addr_of!(TAIL).read();
        if head == tail {
            return None;
        }
        let b = core::ptr::addr_of!(BUF).cast::<u8>().add(tail).read();
        core::ptr::addr_of_mut!(TAIL).write((tail + 1) % CAP);
        Some(b)
    }
}

fn ascii(sc: u8) -> u8 {
    match sc {
        0x1C => b'\n',
        0x0E => 0x08,
        0x39 => b' ',
        0x02 => b'1',
        0x03 => b'2',
        0x04 => b'3',
        0x05 => b'4',
        0x06 => b'5',
        0x07 => b'6',
        0x08 => b'7',
        0x09 => b'8',
        0x0A => b'9',
        0x0B => b'0',
        0x10 => b'q',
        0x11 => b'w',
        0x12 => b'e',
        0x13 => b'r',
        0x14 => b't',
        0x15 => b'y',
        0x16 => b'u',
        0x17 => b'i',
        0x18 => b'o',
        0x19 => b'p',
        0x1E => b'a',
        0x1F => b's',
        0x20 => b'd',
        0x21 => b'f',
        0x22 => b'g',
        0x23 => b'h',
        0x24 => b'j',
        0x25 => b'k',
        0x26 => b'l',
        0x2C => b'z',
        0x2D => b'x',
        0x2E => b'c',
        0x2F => b'v',
        0x30 => b'b',
        0x31 => b'n',
        0x32 => b'm',
        _ => 0,
    }
}

/// IRQ1. Drain one byte, EOI master.
#[unsafe(no_mangle)]
pub extern "C" fn kbd_tick() {
    unsafe {
        if inb(STAT) & 1 != 0 {
            let sc = inb(DATA);
            if core::ptr::addr_of!(EXT).read() {
                core::ptr::addr_of_mut!(EXT).write(false);
            } else if sc == 0xE0 {
                core::ptr::addr_of_mut!(EXT).write(true);
            } else if sc & 0x80 == 0 {
                push(ascii(sc));
            }
        }
        outb(0x20, 0x20);
    }
}

/// House `read` fd 0. Blocks until a byte when the 8042 is live.
pub fn read(buf: *mut u8, len: u64) -> u64 {
    if buf.is_null() || len == 0 || len > (1 << 20) {
        return err(EPERM);
    }
    if !unsafe { core::ptr::addr_of!(LIVE).read() } {
        return err(EAGAIN);
    }
    let n = drain(buf, len);
    if n > 0 {
        return n;
    }
    unsafe {
        asm!("sti", options(nomem, nostack, preserves_flags));
    }
    loop {
        if let Some(b) = pop() {
            unsafe {
                buf.write_volatile(b);
            }
            let rest = drain(unsafe { buf.add(1) }, len.saturating_sub(1));
            return 1 + rest;
        }
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

fn drain(buf: *mut u8, len: u64) -> u64 {
    let mut i = 0u64;
    while i < len {
        let Some(b) = pop() else {
            break;
        };
        unsafe {
            buf.add(i as usize).write_volatile(b);
        }
        i += 1;
    }
    i
}

#[allow(dead_code)]
pub fn live() -> bool {
    unsafe { core::ptr::addr_of!(LIVE).read() }
}
