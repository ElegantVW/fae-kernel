//! PIT timer (BIOS path only): 100 Hz ticks so `sleep`/`time` mean ms.
//! EFI/crutch paths never call [`init`] — there `sleep` is a stub and
//! `time` falls back to raw `rdtsc`. The gate stays honest either way.

use core::arch::asm;

static mut TICKS: u64 = 0;
static mut ACTIVE: bool = false;

#[inline]
unsafe fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags))
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

fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// Program PIT ch0 @100 Hz, zero ticks, unmask IRQ0. Call once, IF clear.
pub fn init() {
    unsafe {
        outb(0x43, 0x36);
        let div: u16 = 11932; // 1193182 / 100
        outb(0x40, (div & 0xFF) as u8);
        outb(0x40, (div >> 8) as u8);
        core::ptr::addr_of_mut!(TICKS).write(0);
        let m = inb(0x21) & !0x01;
        outb(0x21, m);
        core::ptr::addr_of_mut!(ACTIVE).write(true);
    }
}

/// IRQ0 path. Keep tiny: count, EOI master, return.
#[unsafe(no_mangle)]
pub extern "C" fn timer_tick() {
    unsafe {
        let t = core::ptr::addr_of_mut!(TICKS);
        t.write(t.read().wrapping_add(1));
        outb(0x20, 0x20);
    }
}

pub fn active() -> bool {
    unsafe { core::ptr::addr_of!(ACTIVE).read() }
}

pub fn ticks() -> u64 {
    unsafe { core::ptr::addr_of!(TICKS).read() }
}

/// ms since timer start (BIOS) or raw ticks (EFI/crutch fallback).
pub fn ms() -> u64 {
    if active() {
        ticks().wrapping_mul(10)
    } else {
        rdtsc()
    }
}

/// Block ~ms (BIOS). Elsewhere returns at once — honest stub.
pub fn sleep_ms(ms: u64) {
    if !active() {
        return;
    }
    let target = ticks().wrapping_add(ms.div_ceil(10));
    unsafe {
        asm!("sti", options(nomem, nostack, preserves_flags));
    }
    while ticks() < target {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
