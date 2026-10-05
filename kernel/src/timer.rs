//! PIT timer (BIOS path only): 100 Hz ticks so `sleep`/`time` mean ms.
//! EFI/crutch paths never call [`init`] — there `sleep` is a stub and
//! `time` falls back to raw `rdtsc`. The gate stays honest either way.
//!
//! Virtual-wire (LAPIC LINT0 as ExtINT) is programmed in [`wire`], post-well:
//! the APIC window only exists in our own tables, and `init` runs on the
//! firmware tables. SeaBIOS-alike boards arrive with the LAPIC enabled; our
//! column did not, so `wire` sets EN itself (miss it and the MMIO is a
//! whisper — the self-test FAIL 12 catches exactly that).

use core::arch::asm;

static mut TICKS: u64 = 0;
static mut ACTIVE: bool = false;

/// Readback of virtual-wire programming, for the self-test (never silent).
#[allow(dead_code)]
static mut SVR_SEEN: u32 = 0;
#[allow(dead_code)]
static mut LINT_SEEN: u32 = 0;
#[allow(dead_code)]
static mut APIC_BASE_SEEN: u64 = 0;

const LAPIC: u64 = 0xFEE0_0000;
const SVR_OFF: usize = 0xF0 / 4;
const LINT0_OFF: usize = 0x350 / 4;

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

/// Program PIT ch0 @100 Hz, zero ticks, unmask IRQ0. Call once, IF clear,
/// pre-well (ports only — no mapping needed).
pub fn init() {
    unsafe {
        outb(0x43, 0x36);
        let div: u16 = 11932; // 1193182 / 100
        outb(0x40, (div & 0xFF) as u8);
        outb(0x40, (div >> 8) as u8);
        core::ptr::addr_of_mut!(TICKS).write_volatile(0);
        let m = inb(0x21) & !0x01;
        outb(0x21, m);
    }
}

/// Virtual-wire + ACTIVE. Call post-well (after_cup): enable the LAPIC,
/// set LINT0 ExtINT, stash readback. Without the window the timer stays a
/// stub (ACTIVE false) — sleep returns at once, time falls back to rdtsc.
pub fn wire() {
    unsafe {
        let mut live = false;
        if crate::mm::ap_mapped() {
            let ap = LAPIC as *mut u32;
            let mut lo = 0u32;
            let mut hi = 0u32;
            asm!("rdmsr", in("ecx") 0x1Bu32, out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
            let mut base = ((hi as u64) << 32) | (lo as u64);
            base |= 1 << 11;
            asm!("wrmsr", in("ecx") 0x1Bu32, in("eax") base as u32, in("edx") (base >> 32) as u32, options(nomem, nostack, preserves_flags));
            core::ptr::addr_of_mut!(APIC_BASE_SEEN).write_volatile(base);
            // EN was just set, so this runs — the readback in the self-test
            // (FAIL 9/10) is the proof, not the branch.
            ap.add(SVR_OFF)
                .write_volatile(ap.add(SVR_OFF).read_volatile() | 0x100);
            ap.add(LINT0_OFF).write_volatile(0x700);
            core::ptr::addr_of_mut!(SVR_SEEN).write_volatile(ap.add(SVR_OFF).read_volatile());
            core::ptr::addr_of_mut!(LINT_SEEN).write_volatile(ap.add(LINT0_OFF).read_volatile());
            live = true;
        }
        core::ptr::addr_of_mut!(ACTIVE).write_volatile(live);
    }
}

/// IRQ0 path. Keep tiny: count, EOI master, return.
#[unsafe(no_mangle)]
pub extern "sysv64" fn timer_tick() {
    unsafe {
        let t = core::ptr::addr_of_mut!(TICKS);
        // Volatile both ways: this races the hlt-loop reader in sleep_ms.
        t.write_volatile(t.read_volatile().wrapping_add(1));
        outb(0x20, 0x20);
    }
}

pub fn active() -> bool {
    unsafe { core::ptr::addr_of!(ACTIVE).read() }
}

/// Virtual-wire readback for the self-test.
#[allow(dead_code)]
pub fn svr_seen() -> u32 {
    unsafe { core::ptr::addr_of!(SVR_SEEN).read_volatile() }
}

/// Virtual-wire readback for the self-test.
#[allow(dead_code)]
pub fn lint_seen() -> u32 {
    unsafe { core::ptr::addr_of!(LINT_SEEN).read_volatile() }
}

/// IA32_APIC_BASE as seen at wire time, for the self-test.
#[allow(dead_code)]
pub fn apic_base_seen() -> u64 {
    unsafe { core::ptr::addr_of!(APIC_BASE_SEEN).read_volatile() }
}

pub fn ticks() -> u64 {
    unsafe { core::ptr::addr_of!(TICKS).read_volatile() }
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
