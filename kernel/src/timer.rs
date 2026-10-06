//! One clock: `sleep` / `time` are milliseconds on every path that arms.
//!
//! BIOS still programs the PIT at 100 Hz ([`init`], pre-well) so IRQ0 +
//! `hlt` can wait. EFI never calls [`init`]. After the well, [`arm`]
//! takes HPET (or a polled PIT) and calibrates TSC; then `ms()` is real
//! ms and `sleep` waits. A dead source is `no tick` — never a silent sleep.
//!
//! Virtual-wire (LAPIC LINT0 as ExtINT) is programmed in [`wire`], post-well:
//! the APIC window only exists in our own tables, and `init` runs on the
//! firmware tables. SeaBIOS-alike boards arrive with the LAPIC enabled; our
//! column did not, so `wire` sets EN itself (miss it and the MMIO is a
//! whisper — the self-test FAIL 12 catches exactly that).

use core::arch::asm;

static mut TICKS: u64 = 0;
static mut ACTIVE: bool = false;
static mut PIT: bool = false;
static mut CLOCK: bool = false;
static mut TSC0: u64 = 0;
static mut TSC_PER_MS: u64 = 0;

/// Readback of virtual-wire programming, for the self-test (never silent).
#[allow(dead_code)]
static mut SVR_SEEN: u32 = 0;
#[allow(dead_code)]
static mut LINT_SEEN: u32 = 0;
#[allow(dead_code)]
static mut APIC_BASE_SEEN: u64 = 0;

const LAPIC: u64 = 0xFEE0_0000;
const HPET: u64 = 0xFED0_0000;
const SVR_OFF: usize = 0xF0 / 4;
const LINT0_OFF: usize = 0x350 / 4;
const CAL_SPINS: u32 = 200_000_000;
const PIT_10MS: u16 = 11932;

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
        core::ptr::addr_of_mut!(PIT).write(true);
        let m = inb(0x21) & !0x01;
        outb(0x21, m);
    }
}

/// PIT programmed and IRQ0 unmasked. EFI skips [`init`], so `hlt` in
/// `read` would never wake — poll + `pause` there instead.
pub fn pit_armed() -> bool {
    unsafe { core::ptr::addr_of!(PIT).read() }
}

fn r64(a: u64) -> u64 {
    unsafe { (a as *const u64).read_volatile() }
}

fn w64(a: u64, v: u64) {
    unsafe { (a as *mut u64).write_volatile(v) }
}

fn tsc_sane(per_ms: u64) -> bool {
    (10_000..50_000_000).contains(&per_ms)
}

fn adopt(per_ms: u64) -> bool {
    if !tsc_sane(per_ms) {
        return false;
    }
    unsafe {
        core::ptr::addr_of_mut!(TSC_PER_MS).write(per_ms);
        core::ptr::addr_of_mut!(TSC0).write(rdtsc());
        core::ptr::addr_of_mut!(CLOCK).write(true);
    }
    true
}

fn hpet_live() -> Option<u64> {
    if !crate::mm::ap_mapped() {
        crate::mm::map_uc(HPET, 0x400);
    }
    let cap = r64(HPET);
    let period = cap >> 32;
    let rev = cap & 0xFF;
    if rev == 0 || !(1_000_000..=100_000_000).contains(&period) {
        return None;
    }
    let cfg = r64(HPET + 0x10);
    w64(HPET + 0x10, cfg | 1);
    let c0 = r64(HPET + 0xF0);
    let mut n = 0u32;
    while n < 200_000 {
        if r64(HPET + 0xF0) != c0 {
            return Some(period);
        }
        n = n.saturating_add(1);
        core::hint::spin_loop();
    }
    None
}

fn cal_hpet(period: u64) -> bool {
    let want = 10_000_000_000_000u64 / period;
    if want == 0 {
        return false;
    }
    let t0 = rdtsc();
    let c0 = r64(HPET + 0xF0);
    let mut n = 0u32;
    while n < CAL_SPINS {
        if r64(HPET + 0xF0).wrapping_sub(c0) >= want {
            let dt = rdtsc().wrapping_sub(t0);
            return adopt(dt / 10);
        }
        n = n.saturating_add(1);
        core::hint::spin_loop();
    }
    false
}

fn cal_irq() -> bool {
    if !pit_armed() || !active() {
        return false;
    }
    unsafe {
        asm!("sti", options(nomem, nostack, preserves_flags));
    }
    let k0 = ticks();
    let t0 = rdtsc();
    let mut n = 0u32;
    while ticks().wrapping_sub(k0) < 1 {
        n = n.saturating_add(1);
        if n >= CAL_SPINS {
            return false;
        }
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
    let dt = rdtsc().wrapping_sub(t0);
    adopt(dt / 10)
}

fn pit_latch() -> u16 {
    unsafe {
        outb(0x43, 0x00);
        let lo = inb(0x40) as u16;
        let hi = inb(0x40) as u16;
        lo | (hi << 8)
    }
}

fn cal_pit_poll() -> bool {
    unsafe {
        outb(0x43, 0x30);
        outb(0x40, (PIT_10MS & 0xFF) as u8);
        outb(0x40, (PIT_10MS >> 8) as u8);
    }
    let t0 = rdtsc();
    let mut saw = false;
    let mut n = 0u32;
    while n < CAL_SPINS {
        let c = pit_latch();
        if c != 0 && c <= PIT_10MS {
            saw = true;
        }
        if saw && (c == 0 || c > PIT_10MS) {
            let dt = rdtsc().wrapping_sub(t0);
            return adopt(dt / 10);
        }
        n = n.saturating_add(1);
        core::hint::spin_loop();
    }
    false
}

/// Post-well: HPET or polled PIT, then TSC so `time`/`sleep` are ms.
pub fn arm() {
    unsafe {
        core::ptr::addr_of_mut!(CLOCK).write(false);
        core::ptr::addr_of_mut!(TSC_PER_MS).write(0);
    }
    if let Some(period) = hpet_live() {
        if cal_hpet(period) {
            return;
        }
    }
    if cal_irq() {
        return;
    }
    if !pit_armed() {
        let _ = cal_pit_poll();
    }
}

pub fn clock_live() -> bool {
    unsafe { core::ptr::addr_of!(CLOCK).read() }
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

/// Milliseconds since [`arm`], when the clock is live. BIOS IRQ ticks if
/// the TSC never calibrated. Raw `rdtsc` only when both are dark.
pub fn ms() -> u64 {
    let per = unsafe { core::ptr::addr_of!(TSC_PER_MS).read() };
    if clock_live() && per != 0 {
        let t0 = unsafe { core::ptr::addr_of!(TSC0).read() };
        rdtsc().wrapping_sub(t0) / per
    } else if active() {
        ticks().wrapping_mul(10)
    } else {
        rdtsc()
    }
}

/// Block ~ms when the clock is live. IRQ0 `hlt` on BIOS; TSC `pause` on EFI.
/// Dead clock returns at once.
pub fn sleep_ms(ms: u64) {
    if ms == 0 {
        return;
    }
    let per = unsafe { core::ptr::addr_of!(TSC_PER_MS).read() };
    if clock_live() && per != 0 {
        let start = rdtsc();
        let delta = ms.saturating_mul(per);
        if pit_armed() && active() {
            unsafe {
                asm!("sti", options(nomem, nostack, preserves_flags));
            }
        }
        while rdtsc().wrapping_sub(start) < delta {
            if pit_armed() && active() {
                unsafe {
                    asm!("hlt", options(nomem, nostack, preserves_flags));
                }
            } else {
                core::hint::spin_loop();
            }
        }
        return;
    }
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
