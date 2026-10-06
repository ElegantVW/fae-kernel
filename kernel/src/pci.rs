//! Legacy PCI config (`0xCF8` / `0xCFC`). First sitting: find xHCI BARs.

use core::arch::asm;

const CFG_ADDR: u16 = 0xCF8;
const CFG_DATA: u16 = 0xCFC;

const CLASS_SERIAL: u8 = 0x0C;
const SUB_USB: u8 = 0x03;
const IF_XHCI: u8 = 0x30;

#[derive(Clone, Copy)]
pub struct XhciBar {
    pub bar: u64,
    pub len: u64,
}

#[inline]
unsafe fn outl(port: u16, val: u32) {
    unsafe {
        asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack, preserves_flags));
    }
}

#[inline]
unsafe fn inl(port: u16) -> u32 {
    let val: u32;
    unsafe {
        asm!("in eax, dx", out("eax") val, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    val
}

fn addr(bus: u8, dev: u8, fun: u8, off: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((fun as u32) << 8)
        | (off as u32 & 0xFC)
}

fn read32(bus: u8, dev: u8, fun: u8, off: u8) -> u32 {
    unsafe {
        outl(CFG_ADDR, addr(bus, dev, fun, off));
        inl(CFG_DATA)
    }
}

fn write32(bus: u8, dev: u8, fun: u8, off: u8, val: u32) {
    unsafe {
        outl(CFG_ADDR, addr(bus, dev, fun, off));
        outl(CFG_DATA, val);
    }
}

fn is_xhci(bus: u8, dev: u8, fun: u8) -> bool {
    let vendor = read32(bus, dev, fun, 0) & 0xFFFF;
    if vendor == 0xFFFF {
        return false;
    }
    let cc = read32(bus, dev, fun, 0x08);
    let class = (cc >> 24) as u8;
    let sub = (cc >> 16) as u8;
    let iff = (cc >> 8) as u8;
    class == CLASS_SERIAL && sub == SUB_USB && iff == IF_XHCI
}

fn bar0(bus: u8, dev: u8, fun: u8) -> Option<(u64, u64)> {
    let raw0 = read32(bus, dev, fun, 0x10);
    if raw0 & 1 != 0 {
        return None;
    }
    let is64 = (raw0 & 6) == 4;
    let raw1 = if is64 {
        read32(bus, dev, fun, 0x14)
    } else {
        0
    };
    write32(bus, dev, fun, 0x10, 0xFFFF_FFFF);
    if is64 {
        write32(bus, dev, fun, 0x14, 0xFFFF_FFFF);
    }
    let size0 = read32(bus, dev, fun, 0x10);
    let size1 = if is64 {
        read32(bus, dev, fun, 0x14)
    } else {
        0
    };
    write32(bus, dev, fun, 0x10, raw0);
    if is64 {
        write32(bus, dev, fun, 0x14, raw1);
    }
    let bar = (raw0 as u64 & 0xFFFF_FFF0) | ((raw1 as u64) << 32);
    let mut mask = (size0 as u64 & 0xFFFF_FFF0) | ((size1 as u64) << 32);
    if !is64 {
        mask &= 0xFFFF_FFFF;
    }
    if mask == 0 || bar == 0 {
        return None;
    }
    let len = (!mask).wrapping_add(1);
    if !is64 {
        Some((bar, len & 0xFFFF_FFFF))
    } else {
        Some((bar, len))
    }
}

fn enable_mem_master(bus: u8, dev: u8, fun: u8) {
    let cmd = read32(bus, dev, fun, 0x04);
    write32(bus, dev, fun, 0x04, cmd | 0x06);
}

/// Fill `out` with xHCI MMIO windows. Returns how many were stored.
pub fn iter_xhci(out: &mut [XhciBar]) -> usize {
    let mut n = 0usize;
    if out.is_empty() {
        return 0;
    }
    let mut bus = 0u8;
    loop {
        let mut dev = 0u8;
        loop {
            let vendor = read32(bus, dev, 0, 0) & 0xFFFF;
            let funcs = if vendor == 0xFFFF {
                0u8
            } else if (read32(bus, dev, 0, 0x0C) >> 16) & 0x80 != 0 {
                8
            } else {
                1
            };
            let mut fun = 0u8;
            while fun < funcs {
                if is_xhci(bus, dev, fun) {
                    if let Some((bar, len)) = bar0(bus, dev, fun) {
                        enable_mem_master(bus, dev, fun);
                        out[n] = XhciBar { bar, len };
                        n += 1;
                        if n >= out.len() {
                            return n;
                        }
                    }
                }
                fun = fun.saturating_add(1);
            }
            if dev == 31 {
                break;
            }
            dev = dev.saturating_add(1);
        }
        if bus == 255 {
            break;
        }
        bus = bus.saturating_add(1);
    }
    n
}
