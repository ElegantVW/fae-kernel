//! Storage dispatcher — name every partition, mount none but FAT.
//!
//! After MSC answers READ CAPACITY, Kindling walks MBR or GPT and paints
//! each slice: `fat` / `kindlog` / `empty` / `other`. FAT stays the glean
//! arm (ESP). KINDLOG magic `KLOG` is recognized this sitting, not mounted.
//! No writes.

use crate::glass;
use crate::start::serial_print;

const MAX_PARTS: usize = 8;
const KINDLOG_MAGIC: [u8; 4] = *b"KLOG";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Fat,
    Kindlog,
    Empty,
    Other,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Fat => "fat",
            Kind::Kindlog => "kindlog",
            Kind::Empty => "empty",
            Kind::Other => "other",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    Miss,
    None,
    Disk,
    Mbr,
    Gpt,
}

impl Table {
    fn word(self) -> &'static str {
        match self {
            Table::Miss => "miss",
            Table::None => "none",
            Table::Disk => "disk",
            Table::Mbr => "mbr",
            Table::Gpt => "gpt",
        }
    }
}

#[derive(Clone, Copy)]
struct Part {
    start: u32,
    mbr_ty: u8,
    gpt: bool,
    guid: [u8; 16],
    kind: Kind,
}

struct Map {
    blk: u32,
    n_lba: u32,
    table: Table,
    n: u8,
    parts: [Part; MAX_PARTS],
}

const EMPTY_PART: Part = Part {
    start: 0,
    mbr_ty: 0,
    gpt: false,
    guid: [0; 16],
    kind: Kind::Other,
};

static mut MAP: Map = Map {
    blk: 0,
    n_lba: 0,
    table: Table::None,
    n: 0,
    parts: [EMPTY_PART; MAX_PARTS],
};

fn map_mut() -> &'static mut Map {
    unsafe { &mut *core::ptr::addr_of_mut!(MAP) }
}

fn map() -> &'static Map {
    unsafe { &*core::ptr::addr_of!(MAP) }
}

fn r8(a: u64) -> u8 {
    unsafe { (a as *const u8).read_volatile() }
}

fn r32le(a: u64) -> u32 {
    r8(a) as u32 | (r8(a + 1) as u32) << 8 | (r8(a + 2) as u32) << 16 | (r8(a + 3) as u32) << 24
}

fn lba32_at(a: u64) -> Option<u32> {
    let lo = r32le(a);
    let hi = r32le(a + 4);
    if hi != 0 || lo == 0 { None } else { Some(lo) }
}

fn guid_at(p: u64) -> [u8; 16] {
    let mut g = [0u8; 16];
    let mut i = 0usize;
    while i < 16 {
        g[i] = r8(p + i as u64);
        i += 1;
    }
    g
}

fn guid_zero(g: &[u8; 16]) -> bool {
    let mut i = 0usize;
    while i < 16 {
        if g[i] != 0 {
            return false;
        }
        i += 1;
    }
    true
}

fn sector_zero(p: u64) -> bool {
    let mut i = 0u64;
    while i < 512 {
        if r8(p + i) != 0 {
            return false;
        }
        i = i.saturating_add(1);
    }
    true
}

/// FAT boot sector: 0x55AA plus `FAT32` or `FAT1`.
pub(crate) fn looks_fat(p: u64) -> bool {
    if r8(p + 510) != 0x55 || r8(p + 511) != 0xAA {
        return false;
    }
    let fat32 = r8(p + 82) == b'F'
        && r8(p + 83) == b'A'
        && r8(p + 84) == b'T'
        && r8(p + 85) == b'3'
        && r8(p + 86) == b'2';
    let fat16 = r8(p + 54) == b'F' && r8(p + 55) == b'A' && r8(p + 56) == b'T';
    fat32 || fat16
}

fn looks_kindlog(p: u64) -> bool {
    r8(p) == KINDLOG_MAGIC[0]
        && r8(p + 1) == KINDLOG_MAGIC[1]
        && r8(p + 2) == KINDLOG_MAGIC[2]
        && r8(p + 3) == KINDLOG_MAGIC[3]
}

fn classify(buf: u64) -> Kind {
    if looks_kindlog(buf) {
        Kind::Kindlog
    } else if looks_fat(buf) {
        Kind::Fat
    } else if sector_zero(buf) {
        Kind::Empty
    } else {
        Kind::Other
    }
}

fn push(p: Part) {
    let m = map_mut();
    if (m.n as usize) >= MAX_PARTS {
        return;
    }
    m.parts[m.n as usize] = p;
    m.n = m.n.saturating_add(1);
}

fn classify_at(start: u32, mbr_ty: u8, gpt: bool, guid: [u8; 16]) {
    let kind = match crate::usb::read_sec(start) {
        Some(buf) => classify(buf),
        None => Kind::Other,
    };
    push(Part {
        start,
        mbr_ty,
        gpt,
        guid,
        kind,
    });
}

struct Slot {
    start: u32,
    ty: u8,
}

fn mbr_slots(buf: u64) -> ([Slot; 4], bool) {
    let mut slots = [
        Slot { start: 0, ty: 0 },
        Slot { start: 0, ty: 0 },
        Slot { start: 0, ty: 0 },
        Slot { start: 0, ty: 0 },
    ];
    let mut gpt = false;
    let mut i = 0usize;
    while i < 4 {
        let e = buf + 446 + i as u64 * 16;
        let ty = r8(e + 4);
        let start = r32le(e + 8);
        if ty == 0xEE {
            gpt = true;
        }
        slots[i] = Slot { start, ty };
        i += 1;
    }
    (slots, gpt)
}

fn probe_gpt() {
    let Some(buf) = crate::usb::read_sec(1) else {
        map_mut().table = Table::Miss;
        return;
    };
    if r8(buf) != b'E'
        || r8(buf + 1) != b'F'
        || r8(buf + 2) != b'I'
        || r8(buf + 3) != b' '
        || r8(buf + 4) != b'P'
        || r8(buf + 5) != b'A'
        || r8(buf + 6) != b'R'
        || r8(buf + 7) != b'T'
    {
        map_mut().table = Table::None;
        return;
    }
    let part_lba = lba32_at(buf + 72).unwrap_or(2);
    let nent = r32le(buf + 80).min(32);
    let esz = r32le(buf + 84);
    if esz != 128 || part_lba == 0 {
        map_mut().table = Table::None;
        return;
    }
    map_mut().table = Table::Gpt;
    let per = 512 / 128;
    let mut raw: [(u32, [u8; 16]); MAX_PARTS] = [(0, [0; 16]); MAX_PARTS];
    let mut n = 0usize;
    let mut idx = 0u32;
    let mut buf = 0u64;
    while idx < nent && n < MAX_PARTS {
        if idx.is_multiple_of(per) {
            match crate::usb::read_sec(part_lba.saturating_add(idx / per)) {
                Some(b) => buf = b,
                None => break,
            }
        }
        if buf == 0 {
            break;
        }
        let e = buf + (idx % per) as u64 * 128;
        let guid = guid_at(e);
        if !guid_zero(&guid)
            && let Some(s) = lba32_at(e + 32)
        {
            raw[n] = (s, guid);
            n += 1;
        }
        idx = idx.saturating_add(1);
    }
    let mut k = 0usize;
    while k < n {
        classify_at(raw[k].0, 0, true, raw[k].1);
        k += 1;
    }
}

/// Walk the live MSC disk. Call after the USB host is installed.
pub fn probe() {
    let m = map_mut();
    m.blk = 0;
    m.n_lba = 0;
    m.table = Table::None;
    m.n = 0;
    let mut i = 0usize;
    while i < MAX_PARTS {
        m.parts[i] = EMPTY_PART;
        i += 1;
    }
    let Some((blk, n_lba)) = crate::usb::disk_geom() else {
        return;
    };
    m.blk = blk;
    m.n_lba = n_lba;
    if blk != 512 {
        return;
    }
    let Some(buf) = crate::usb::read_sec(0) else {
        m.table = Table::Miss;
        return;
    };
    if looks_fat(buf) {
        m.table = Table::Disk;
        push(Part {
            start: 0,
            mbr_ty: 0,
            gpt: false,
            guid: [0; 16],
            kind: Kind::Fat,
        });
        return;
    }
    if r8(buf + 510) != 0x55 || r8(buf + 511) != 0xAA {
        return;
    }
    let (slots, gpt) = mbr_slots(buf);
    if gpt {
        probe_gpt();
        if map().n == 0 && map().table != Table::Gpt {
            let mut i = 0usize;
            while i < 4 {
                if slots[i].ty != 0 && slots[i].ty != 0xEE && slots[i].start != 0 {
                    map_mut().table = Table::Mbr;
                    classify_at(slots[i].start, slots[i].ty, false, [0; 16]);
                }
                i += 1;
            }
        }
        return;
    }
    map_mut().table = Table::Mbr;
    let mut i = 0usize;
    while i < 4 {
        if slots[i].ty != 0 && slots[i].start != 0 {
            classify_at(slots[i].start, slots[i].ty, false, [0; 16]);
        }
        i += 1;
    }
}

/// First FAT partition start, if the walk named one.
pub fn first_fat() -> Option<u32> {
    let m = map();
    let mut i = 0u8;
    while i < m.n {
        if m.parts[i as usize].kind == Kind::Fat {
            return Some(m.parts[i as usize].start);
        }
        i = i.saturating_add(1);
    }
    None
}

fn put_str(out: &mut [u8], mut i: usize, s: &[u8]) -> usize {
    let mut k = 0usize;
    while k < s.len() && i < out.len() {
        out[i] = s[k];
        i += 1;
        k += 1;
    }
    i
}

fn put_dec(out: &mut [u8], mut i: usize, mut n: u32) -> usize {
    if i >= out.len() {
        return i;
    }
    if n == 0 {
        out[i] = b'0';
        return i + 1;
    }
    let mut tmp = [0u8; 10];
    let mut t = 10usize;
    while n > 0 {
        t -= 1;
        tmp[t] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    while t < 10 && i < out.len() {
        out[i] = tmp[t];
        i += 1;
        t += 1;
    }
    i
}

fn put_hex2(out: &mut [u8], i: usize, v: u8) -> usize {
    const H: &[u8; 16] = b"0123456789abcdef";
    if i + 1 >= out.len() {
        return i;
    }
    out[i] = H[(v >> 4) as usize];
    out[i + 1] = H[(v & 0xf) as usize];
    i + 2
}

fn put_hex8(out: &mut [u8], mut i: usize, v: u32) -> usize {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = 28i32;
    while s >= 0 && i < out.len() {
        out[i] = H[((v >> s) & 0xf) as usize];
        i += 1;
        s -= 4;
    }
    i
}

fn speak(buf: &[u8]) {
    let Ok(s) = core::str::from_utf8(buf) else {
        return;
    };
    serial_print("kindling: ");
    serial_print(s);
    serial_print("\n");
    glass::put_bytes(s.as_ptr(), s.len() as u64);
    glass::put_bytes(b"\n".as_ptr(), 1);
}

/// Glass/serial dump after `msc`. No writes.
pub fn paint() {
    let m = map();
    if m.blk == 0 {
        return;
    }
    let mut line = [0u8; 48];
    let mut i = put_str(&mut line, 0, b"store ");
    i = put_str(&mut line, i, m.table.word().as_bytes());
    i = put_str(&mut line, i, b" ");
    i = put_dec(&mut line, i, m.blk);
    i = put_str(&mut line, i, b" ");
    i = put_dec(&mut line, i, m.n_lba);
    speak(&line[..i]);
    let mut p = 0u8;
    while p < m.n {
        let part = m.parts[p as usize];
        let mut line = [0u8; 40];
        let mut i = 0usize;
        if part.gpt {
            let ty = u32::from_le_bytes([part.guid[0], part.guid[1], part.guid[2], part.guid[3]]);
            i = put_hex8(&mut line, i, ty);
        } else {
            i = put_hex2(&mut line, i, part.mbr_ty);
        }
        i = put_str(&mut line, i, b" ");
        i = put_dec(&mut line, i, part.start);
        i = put_str(&mut line, i, b" ");
        i = put_str(&mut line, i, part.kind.word().as_bytes());
        speak(&line[..i]);
        p = p.saturating_add(1);
    }
}
