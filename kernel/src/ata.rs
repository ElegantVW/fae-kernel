//! ATA PIO, master, LBA28 — read + write. Mirrors `ld/cerne-ld.asm`'s rite:
//! select, settle, floating-bus check, command, generous poll, 256 words,
//! SRST + retry. The loader reads before the jump; this writes after it
//! (G2b `stow`). Paved ATA path only — elsewhere the bus floats and every
//! call honestly refuses.

const DATA: u16 = 0x1F0;
const COUNT: u16 = 0x1F2;
const LBA_LO: u16 = 0x1F3;
const LBA_MID: u16 = 0x1F4;
const LBA_HI: u16 = 0x1F5;
const DRIVE: u16 = 0x1F6;
const STATUS: u16 = 0x1F7;
const ALT: u16 = 0x3F6;

const READ_CMD: u8 = 0x20;
const WRITE_CMD: u8 = 0x30;
const FLUSH_CMD: u8 = 0xE7;

const BSY: u8 = 0x80;
const DF: u8 = 0x20;
const ERR: u8 = 0x01;
const DRQ: u8 = 0x08;

const BUDGET: u32 = 0x400000; // polls per sector — spin-up is slow on iron
const TRIES: u8 = 3;
const MAX_SECTORS: u8 = 32; // per call; the gates move one or two

#[inline]
unsafe fn outb(port: u16, val: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags))
    }
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe {
        core::arch::asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack, preserves_flags))
    }
    v
}

#[inline]
unsafe fn outw(port: u16, val: u16) {
    unsafe {
        core::arch::asm!("out dx, ax", in("dx") port, in("ax") val, options(nomem, nostack, preserves_flags))
    }
}

#[inline]
unsafe fn inw(port: u16) -> u16 {
    let v: u16;
    unsafe {
        core::arch::asm!("in ax, dx", out("ax") v, in("dx") port, options(nomem, nostack, preserves_flags))
    }
    v
}

unsafe fn settle() {
    unsafe {
        inb(ALT);
        inb(ALT);
        inb(ALT);
        inb(ALT);
    }
}

unsafe fn srst() {
    unsafe {
        outb(ALT, 0x04);
        inb(ALT);
        inb(ALT);
        outb(ALT, 0x00);
    }
}

/// Wait for the next word-block. True when DRQ stands, no ERR/DF, in budget.
unsafe fn poll() -> bool {
    unsafe {
        let mut left = BUDGET;
        while left > 0 {
            let s = inb(STATUS);
            if s & BSY == 0 {
                if s & (ERR | DF) != 0 {
                    return false;
                }
                if s & DRQ != 0 {
                    return true;
                }
            }
            left -= 1;
        }
        false
    }
}

/// Wait until the drive is idle (!BSY) and uncomplaining. For write tails.
unsafe fn idle() -> bool {
    unsafe {
        let mut left = BUDGET;
        while left > 0 {
            let s = inb(STATUS);
            if s & BSY == 0 {
                return s & (ERR | DF) == 0;
            }
            left -= 1;
        }
        false
    }
}

/// Select master + LBA28. False on a floating bus (nothing behind the port).
unsafe fn select(lba: u32) -> bool {
    unsafe {
        outb(DRIVE, 0xE0 | ((lba >> 24) & 0x0F) as u8);
        settle();
        let s = inb(STATUS);
        if s == 0x00 || s == 0xFF {
            return false;
        }
        outb(COUNT, 0); // placeholder, overwritten below before the command
        outb(LBA_LO, lba as u8);
        outb(LBA_MID, (lba >> 8) as u8);
        outb(LBA_HI, (lba >> 16) as u8);
        true
    }
}

fn command(lba: u32, count: u8, cmd: u8, buf: *mut u8, write: bool) -> bool {
    if count == 0 || count > MAX_SECTORS || buf.is_null() {
        return false;
    }
    unsafe {
        let mut t = TRIES;
        while t > 0 {
            if !select(lba) {
                return false; // floating bus — retrying an absence is noise
            }
            outb(COUNT, count);
            outb(STATUS, cmd);
            let mut ok = true;
            let mut p = buf;
            for _ in 0..count {
                if !poll() {
                    ok = false;
                    break;
                }
                for _ in 0..256 {
                    if write {
                        outw(DATA, (p as *const u16).read_volatile());
                    } else {
                        (p as *mut u16).write_volatile(inw(DATA));
                    }
                    p = p.add(2);
                }
            }
            if ok {
                if write {
                    // Tail: idle, flush the write cache for real iron, idle.
                    if !idle() {
                        ok = false;
                    } else {
                        outb(STATUS, FLUSH_CMD);
                        ok = idle();
                    }
                }
                if ok {
                    return true;
                }
            }
            t -= 1;
            if t > 0 {
                srst();
            }
        }
        false
    }
}

/// Read `count` sectors at `lba` to `dst`. False refuses, never half-fills
/// beyond the failing sector (callers re-verify anyway).
pub fn read_sectors(lba: u32, count: u8, dst: *mut u8) -> bool {
    command(lba, count, READ_CMD, dst, false)
}

/// Write `count` sectors at `lba` from `src`, cache-flushed. False refuses.
pub fn write_sectors(lba: u32, count: u8, src: *const u8) -> bool {
    command(lba, count, WRITE_CMD, src as *mut u8, true)
}
