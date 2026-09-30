//! Kindling's own GDT. After this, CS is 0x08 — not firmware 0x18.
//! Ring-3 ready: UCODE 0x18 / UDATA 0x20 (DPL 3) + TSS 0x28 carrying
//! RSP0 (kernel cup top, for CPL3 → CPL0 switches) and IST1 (double-fault).

use core::arch::asm;

#[repr(C, packed)]
struct GdtPtr {
    limit: u16,
    base: u64,
}

// TSS as bytes (RSP0 sits at offset 4 — repr(C) would pad it).
static mut TSS: [u8; 104] = [0; 104];
static mut DF_STACK: [u8; 8192] = [0; 8192];

fn tss_write64(off: usize, v: u64) {
    unsafe {
        let p = core::ptr::addr_of_mut!(TSS)
            .cast::<u8>()
            .add(off)
            .cast::<u64>();
        p.write_unaligned(v);
    }
}

fn tss_write16(off: usize, v: u16) {
    unsafe {
        let p = core::ptr::addr_of_mut!(TSS)
            .cast::<u8>()
            .add(off)
            .cast::<u16>();
        p.write_unaligned(v);
    }
}

fn tss_desc(base: u64, limit: u32) -> (u64, u64) {
    let lo = (limit as u64 & 0xFFFF)
        | ((base & 0xFFFF) << 16)
        | (((base >> 16) & 0xFF) << 32)
        | (0x89 << 40)
        | (((limit as u64 >> 16) & 0xF) << 48)
        | (((base >> 24) & 0xFF) << 56);
    let hi = base >> 32;
    (lo, hi)
}

static mut GDT: [u64; 7] = [
    0,
    0x00AF_9A00_0000_FFFF, // 0x08 64-bit kernel code
    0x00AF_9200_0000_FFFF, // 0x10 64-bit kernel data
    0x00AF_FA00_0000_FFFF, // 0x18 64-bit user code (DPL 3)
    0x00AF_F200_0000_FFFF, // 0x20 64-bit user data (DPL 3)
    0,                     // 0x28 TSS low (filled at install)
    0,                     // 0x30 TSS high (filled at install)
];

#[allow(dead_code)]
pub const KCODE: u16 = 0x08;
#[allow(dead_code)]
pub const KDATA: u16 = 0x10;
#[allow(dead_code)]
pub const UCODE: u16 = 0x18;
#[allow(dead_code)]
pub const UDATA: u16 = 0x20;
#[allow(dead_code)]
pub const TSS_SEL: u16 = 0x28;

/// CPL3 return selectors (RPL 3).
#[allow(dead_code)]
pub const UCODE_RPL3: u64 = 0x1B;
#[allow(dead_code)]
pub const UDATA_RPL3: u64 = 0x23;

/// Point RSP0 at the kernel cup top. Call after the well is poured,
/// before any CPL3 entry or `sti` with user reachable.
#[allow(dead_code)]
pub fn set_kernel_stack(top: u64) {
    tss_write64(4, top);
}

pub fn install() {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
        // IST1: double-fault stack. IOMAP base = past the end (no bitmap).
        let df_top = (core::ptr::addr_of!(DF_STACK) as u64) + 8192;
        tss_write64(36, df_top);
        tss_write16(102, 104);
        let (lo, hi) = tss_desc(core::ptr::addr_of!(TSS) as u64, 103);
        let gdt = core::ptr::addr_of_mut!(GDT);
        (*gdt)[5] = lo;
        (*gdt)[6] = hi;
        let ptr = GdtPtr {
            limit: 55,
            base: core::ptr::addr_of!(GDT) as u64,
        };
        asm!("lgdt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
        asm!(
            "push {cs}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",
            cs = const 8u64,
            tmp = out(reg) _,
        );
        asm!(
            "mov {0:x}, {d}",
            "mov ds, {0:x}",
            "mov es, {0:x}",
            "mov ss, {0:x}",
            "mov fs, {0:x}",
            "mov gs, {0:x}",
            out(reg) _,
            d = const 0x10u16,
        );
        asm!("ltr ax", in("ax") TSS_SEL, options(nostack, preserves_flags));
    }
}
