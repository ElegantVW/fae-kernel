//! KINDLOG — Kindling-native volume for the book.
//!
//! Superblock magic `KLOG` at partition LBA 0. Seals sit at fixed relative
//! LBAs: `hands` 1–2, `twin` 3–4, `hand` 5. FAT stays the ESP. Writes are
//! MSC WRITE(10)+settle+reread, KINDLING `85C7-AA81` only, never the superblock
//! and never outside this slice. Live is the superblock itself (magic, ver,
//! vol, super `n` ≤ slice secs, xor), cached at probe so enlist does not
//! depend on a second READ(10). Book leaves never fall through to FAT when
//! a KINDLOG slice is named.

use crate::start::serial_print;
use crate::usb;

const MAGIC: [u8; 4] = *b"KLOG";
const VERSION: u32 = 1;
const KINDLING_VOL: u32 = 0x85C7_AA81;
const MIN_SECS: u32 = 6;
const REL_HANDS: u32 = 1;
const REL_TWIN: u32 = 3;
const REL_HAND: u32 = 5;
const HANDS_LEN: u64 = 716;
const TWIN_LEN: u64 = 716;
const HAND_LEN: u64 = 32;

const EPERM: u64 = 1;
const EIO: u64 = 5;
const ENODEV: u64 = 19;

static mut LIT: bool = false;
static mut LOG_START: u32 = 0;
static mut LOG_SECS: u32 = 0;

fn err(n: u64) -> u64 {
    0u64.wrapping_sub(n)
}

fn r8(a: u64) -> u8 {
    unsafe { (a as *const u8).read_volatile() }
}

fn r32le(a: u64) -> u32 {
    r8(a) as u32 | (r8(a + 1) as u32) << 8 | (r8(a + 2) as u32) << 16 | (r8(a + 3) as u32) << 24
}

fn stem(name: &[u8]) -> &[u8] {
    let mut n = name.len();
    if n > 0 && name[n - 1] == 0 {
        n -= 1;
    }
    &name[..n]
}

/// House leaves that live on KINDLOG when the volume is live.
pub fn is_book_leaf(name: &[u8]) -> bool {
    matches!(stem(name), b"hands" | b"twin" | b"hand")
}

fn leaf(name: &[u8]) -> Option<(u32, u64)> {
    match stem(name) {
        b"hands" => Some((REL_HANDS, HANDS_LEN)),
        b"twin" => Some((REL_TWIN, TWIN_LEN)),
        b"hand" => Some((REL_HAND, HAND_LEN)),
        _ => None,
    }
}

fn checksum(ver: u32, vol: u32, secs: u32) -> u32 {
    u32::from_le_bytes(MAGIC) ^ ver ^ vol ^ secs
}

fn super_ok(buf: u64, secs: u32) -> bool {
    if r8(buf) != MAGIC[0]
        || r8(buf + 1) != MAGIC[1]
        || r8(buf + 2) != MAGIC[2]
        || r8(buf + 3) != MAGIC[3]
    {
        return false;
    }
    let ver = r32le(buf + 4);
    let vol = r32le(buf + 8);
    let n = r32le(buf + 12);
    let xor = r32le(buf + 16);
    ver == VERSION
        && vol == KINDLING_VOL
        && n >= MIN_SECS
        && n <= secs
        && xor == checksum(ver, vol, n)
}

/// Probe named a KINDLOG slice (MBR type `0x6c` or magic `KLOG`).
pub fn named() -> bool {
    crate::store::first_kindlog().is_some()
}

fn speak(s: &'static str) {
    serial_print(s);
    crate::glass::put_bytes(s.as_ptr(), s.len() as u64);
}

/// The slice is on the map but the superblock did not check out.
pub fn speak_dark() {
    speak("the log is dark\n");
}

/// Forget a prior super. `store::probe` calls this before the walk.
pub fn reset() {
    unsafe {
        LIT = false;
        LOG_START = 0;
        LOG_SECS = 0;
    }
}

/// Cache a live super from a sector already in hand (probe / refresh).
pub fn remember(start: u32, secs: u32, buf: u64) {
    if start == 0 || secs < MIN_SECS {
        return;
    }
    if !super_ok(buf, secs) {
        return;
    }
    let n = r32le(buf + 12);
    unsafe {
        LIT = true;
        LOG_START = start;
        LOG_SECS = n;
    }
}

/// Cached slice used for writes. Super `n`, not a possibly-wrong MBR count.
pub fn slice() -> Option<(u32, u32)> {
    unsafe {
        if LIT && LOG_START != 0 && LOG_SECS >= MIN_SECS {
            Some((LOG_START, LOG_SECS))
        } else {
            None
        }
    }
}

/// Re-read the super after FAT is claimed. Quiet if already lit.
pub fn refresh() {
    if unsafe { LIT } {
        return;
    }
    let Some((start, secs)) = crate::store::first_kindlog() else {
        return;
    };
    let Some(buf) = usb::read_sec(start) else {
        return;
    };
    remember(start, secs, buf);
}

/// Glass after `fat`: lit when the super checked out, dark when named and not.
pub fn paint_live() {
    if !named() {
        return;
    }
    if live() {
        speak("the log is lit\n");
    } else {
        speak("the log is dark\n");
    }
}

/// Superblock checks out and the slice is big enough for the book.
pub fn live() -> bool {
    if slice().is_some() {
        return true;
    }
    let Some((start, secs)) = crate::store::first_kindlog() else {
        return false;
    };
    if secs < MIN_SECS {
        return false;
    }
    let Some(buf) = usb::read_sec(start) else {
        return false;
    };
    remember(start, secs, buf);
    slice().is_some()
}

fn abs_lba(rel: u32) -> Option<u32> {
    let (start, secs) = slice().or_else(crate::store::first_kindlog)?;
    if rel == 0 || rel >= secs {
        return None;
    }
    Some(start.saturating_add(rel))
}

fn all_zero(buf: u64, n: u64) -> bool {
    let mut i = 0u64;
    while i < n {
        if r8(buf + i) != 0 {
            return false;
        }
        i = i.saturating_add(1);
    }
    true
}

fn copy_out(src: u64, dst: u64, n: u64) {
    let mut i = 0u64;
    while i < n {
        unsafe {
            ((dst + i) as *mut u8).write_volatile(r8(src + i));
        }
        i = i.saturating_add(1);
    }
}

/// Gather a book leaf from KINDLOG. Wax (all-zero) is 0 bytes.
pub fn glean(name: &[u8], buf: u64, len: u64) -> u64 {
    if buf == 0 || len == 0 {
        return err(EPERM);
    }
    if !live() {
        return err(ENODEV);
    }
    let Some((rel, want)) = leaf(name) else {
        return err(EPERM);
    };
    let Some(lba) = abs_lba(rel) else {
        return err(EIO);
    };
    let nsec = if want > 512 { 2u32 } else { 1u32 };
    let mut tmp = [0u8; 1024];
    let mut s = 0u32;
    while s < nsec {
        let Some(src) = usb::read_sec(lba.saturating_add(s)) else {
            return err(EIO);
        };
        let off = (s as u64).saturating_mul(512);
        let mut i = 0u64;
        while i < 512 {
            tmp[(off + i) as usize] = r8(src + i);
            i = i.saturating_add(1);
        }
        s = s.saturating_add(1);
    }
    if all_zero(tmp.as_ptr() as u64, want) {
        return 0;
    }
    let n = len.min(want);
    copy_out(tmp.as_ptr() as u64, buf, n);
    n
}

/// Lay a book leaf onto KINDLOG. Exact measure. Never the superblock.
pub fn stow(name: &[u8], buf: u64, len: u64) -> u64 {
    if buf == 0 || len == 0 {
        return err(EPERM);
    }
    if !live() {
        return err(ENODEV);
    }
    let Some((rel, want)) = leaf(name) else {
        return err(EPERM);
    };
    if len != want {
        return err(EPERM);
    }
    let Some(lba) = abs_lba(rel) else {
        return err(EIO);
    };
    let nsec = if want > 512 { 2u32 } else { 1u32 };
    let mut s = 0u32;
    let mut off = 0u64;
    while s < nsec {
        let mut sec = [0u8; 512];
        let mut i = 0u64;
        while i < 512 && off < len {
            sec[i as usize] = unsafe { ((buf + off) as *const u8).read_volatile() };
            i = i.saturating_add(1);
            off = off.saturating_add(1);
        }
        if !usb::write_kindlog(lba.saturating_add(s), &sec) {
            return err(EIO);
        }
        s = s.saturating_add(1);
    }
    len
}
