//! Cairn — leaves + sparks at 0x100000, laid by the loader (docs/CAIRN.md).
//! The kernel re-verifies everything: KMAP scratch magic, cairn checksum,
//! header, walk bounds, per-leaf xor. Absent or corrupt is refusal, never
//! partial bytes. BIOS loader lays the slot; EFI plants `EFI/BOOT/CAIRN`
//! the same way. Absent is `-ENODEV`.

use crate::start::{hcf, serial_print, serial_u64};

pub const GLEAN: u64 = 8;
pub const STOW: u64 = 9;

pub const CAIRN_RAM: u64 = 0x100000;
const KMAP_RAM: u64 = 0x8400;
const KMAP_MAGIC: u32 = 0x50414D4B;
const CAIR_MAGIC: u32 = 0x52494143;
const MAX_SECTORS: u32 = 1024;
const MAX_LEAVES: u32 = 256;
const MAX_DATA: u64 = 1 << 20;

const EPERM: u64 = 1;
const ENOENT: u64 = 2;
const EIO: u64 = 5;
const ENODEV: u64 = 19;

fn err(n: u64) -> u64 {
    0u64.wrapping_sub(n)
}

fn align_up(x: u64, a: u64) -> u64 {
    (x + a - 1) & !(a - 1)
}

struct Cairn {
    base: u64,
    len: u64,
}

/// Once verified, by base+len. Sparks share the cairn's roof and may spit on
/// their own floorboards (tale's buf lives inside its own record) — so the
/// full-image xor runs ONCE here; the per-leaf xor in `find` still guards
/// every read after that.
static mut SEALED_BASE: u64 = 0;
static mut SEALED_LEN: u64 = 0;
static mut SEALED: bool = false;

/// Plant a cairn already sitting at [`CAIRN_RAM`]. EFI copies `EFI/BOOT/CAIRN`
/// there before ExitBootServices, then calls this so `open` sees a KMAP.
#[allow(dead_code)]
pub fn offer(len: u64) {
    if len == 0 || len % 512 != 0 {
        return;
    }
    let csec = (len / 512) as u32;
    if csec == 0 || csec > MAX_SECTORS {
        return;
    }
    let mut x = 0u32;
    let mut off = 0u64;
    unsafe {
        while off < len {
            x ^= ((CAIRN_RAM + off) as *const u32).read_volatile();
            off += 4;
        }
        let clba = 1u32;
        let csum = clba ^ csec ^ x ^ CAIR_MAGIC;
        let k = KMAP_RAM as *mut u32;
        k.write_volatile(KMAP_MAGIC);
        k.add(1).write_volatile(0);
        k.add(2).write_volatile(0);
        k.add(3).write_volatile(0);
        k.add(4).write_volatile(0);
        k.add(5).write_volatile(clba);
        k.add(6).write_volatile(csec);
        k.add(7).write_volatile(csum);
        core::ptr::addr_of_mut!(SEALED).write(false);
    }
}

/// Locate + verify the cairn via the KMAP scratch the loader left at 0x8400.
fn open() -> Option<Cairn> {
    unsafe {
        if core::ptr::addr_of!(SEALED).read() {
            let base = core::ptr::addr_of!(SEALED_BASE).read();
            let len = core::ptr::addr_of!(SEALED_LEN).read();
            if (base as *const u32).read_volatile() != CAIR_MAGIC {
                return None;
            }
            return Some(Cairn { base, len });
        }
    }
    let c = open_verify()?;
    unsafe {
        core::ptr::addr_of_mut!(SEALED_BASE).write(c.base);
        core::ptr::addr_of_mut!(SEALED_LEN).write(c.len);
        core::ptr::addr_of_mut!(SEALED).write(true);
    }
    Some(c)
}

/// Full verification: KMAP scratch magic, cairn checksum, header shape.
fn open_verify() -> Option<Cairn> {
    unsafe {
        let k = KMAP_RAM as *const u32;
        if k.read_volatile() != KMAP_MAGIC {
            return None;
        }
        let clba = k.add(5).read_volatile();
        let csec = k.add(6).read_volatile();
        let csum = k.add(7).read_volatile();
        if clba == 0 || csec == 0 || csec > MAX_SECTORS {
            return None;
        }
        let len = (csec as u64) * 512;
        let base = CAIRN_RAM;
        let mut x = 0u32;
        let mut off = 0u64;
        while off < len {
            x ^= ((base + off) as *const u32).read_volatile();
            off += 4;
        }
        if (clba ^ csec ^ x ^ CAIR_MAGIC) != csum {
            return None;
        }
        let h = base as *const u32;
        if h.read_volatile() != CAIR_MAGIC {
            return None;
        }
        if h.add(1).read_volatile() != 1 {
            return None;
        }
        if h.add(2).read_volatile() > MAX_LEAVES {
            return None;
        }
        Some(Cairn { base, len })
    }
}

fn read_u32_at(base: u64, off: u64) -> u32 {
    unsafe { ((base + off) as *const u32).read_volatile() }
}

fn read_u8_at(base: u64, off: u64) -> u8 {
    unsafe { ((base + off) as *const u8).read_volatile() }
}

/// Find a record by name + kind. Ok(addr, len) | Err(ENOENT) | Err(EIO).
fn find(c: &Cairn, name: &[u8], kind_want: u32) -> Result<(u64, u64), u64> {
    let n = read_u32_at(c.base, 8);
    let mut off = 12u64;
    for _ in 0..n {
        if off + 8 > c.len {
            return Err(EIO);
        }
        let kind = read_u32_at(c.base, off);
        let nl = read_u8_at(c.base, off + 4) as u64;
        if nl == 0 || nl > 64 {
            return Err(EIO);
        }
        let de = align_up(off + 5 + nl, 4);
        if de + 4 > c.len {
            return Err(EIO);
        }
        let dl = read_u32_at(c.base, de) as u64;
        if dl > MAX_DATA {
            return Err(EIO);
        }
        let da = de + 4;
        let xe = align_up(da + dl, 4);
        if xe + 4 > c.len {
            return Err(EIO);
        }
        if kind == kind_want && nl == name.len() as u64 {
            let mut same = true;
            for i in 0..nl {
                if read_u8_at(c.base, off + 5 + i) != name[i as usize] {
                    same = false;
                    break;
                }
            }
            if same {
                let mut x = 0u32;
                let mut q = 0u64;
                while q < align_up(dl, 4) {
                    x ^= read_u32_at(c.base, da + q);
                    q += 4;
                }
                if x != read_u32_at(c.base, xe) {
                    return Err(EIO);
                }
                return Ok((c.base + da, dl));
            }
        }
        off = xe + 4;
    }
    Err(ENOENT)
}

/// Read a NUL-terminated leaf name (≤64, plain ascii, no `/`).
/// Err(EPERM) on any cheek: null, empty, overlong, unterminated, slashy.
fn take_name(name_ptr: u64) -> Result<([u8; 64], usize), u64> {
    if name_ptr == 0 {
        return Err(EPERM);
    }
    let mut name = [0u8; 64];
    let mut nl = 0usize;
    loop {
        if nl >= 65 {
            return Err(EPERM);
        }
        let b = unsafe { ((name_ptr + nl as u64) as *const u8).read_volatile() };
        if b == 0 {
            break;
        }
        if nl >= 64 || b == b'/' || !(0x20..=0x7E).contains(&b) {
            return Err(EPERM);
        }
        name[nl] = b;
        nl += 1;
    }
    if nl == 0 {
        return Err(EPERM);
    }
    Ok((name, nl))
}

/// Gather a leaf's bytes: `rdi` = name, `rsi` = buf, `rdx` = len (cap 1 MiB).
/// Returns bytes copied (a short read when the leaf is longer is honest, not
/// an error) or `-errno`. v0 trusts mapped RAM for pointers; spawn's realm
/// still lets the kernel copy (CPL0 ignores U/S). User pointer checks wait.
pub fn glean(name_ptr: u64, buf: u64, len: u64) -> u64 {
    if buf == 0 || len == 0 || len > MAX_DATA {
        return err(EPERM);
    }
    let (name, nl) = match take_name(name_ptr) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let Some(c) = open() else {
        return err(ENODEV);
    };
    match find(&c, &name[..nl], 0) {
        Ok((da, dl)) => {
            let n = len.min(dl);
            let mut i = 0u64;
            while i < n {
                let b = unsafe { ((da + i) as *const u8).read_volatile() };
                unsafe { ((buf + i) as *mut u8).write_volatile(b) };
                i += 1;
            }
            n
        }
        Err(e) => err(e),
    }
}

static mut SECTOR_SCRATCH: [u16; 256] = [0; 256]; // 512 B, 2-aligned for the word loop

fn sector_eq(ram: u64, disk: u64) -> bool {
    let mut i = 0u64;
    while i < 512 {
        unsafe {
            if ((ram + i) as *const u8).read_volatile() != ((disk + i) as *const u8).read_volatile()
            {
                return false;
            }
        }
        i += 1;
    }
    true
}

fn cairn_xor(base: u64, len: u64) -> u32 {
    let mut x = 0u32;
    let mut off = 0u64;
    while off < len {
        x ^= unsafe { ((base + off) as *const u32).read_volatile() };
        off += 4;
    }
    x
}

/// Lay bytes down: re-ink a leaf with the same measure (`rdx` must equal the
/// leaf's `datalen` — G2b keeps one shape, growing leaves is later work).
/// Writes the touched cairn sectors back through ATA, re-reads each to prove
/// it landed, rewrites LBA0 with the new cairn sum so the next boot still
/// trusts the cairn, and verifies that too. Returns bytes laid or `-errno`
/// (`EPERM` args/shape/spark, `ENOENT` missing, `ENODEV` no cairn,
/// `EIO` disk error or verify mismatch). Ink is for leaves — sparks refuse.
pub fn stow(name_ptr: u64, buf: u64, len: u64) -> u64 {
    if buf == 0 || len == 0 || len > MAX_DATA {
        return err(EPERM);
    }
    let (name, nl) = match take_name(name_ptr) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let Some(c) = open() else {
        return err(ENODEV);
    };
    let (da, dl) = match find(&c, &name[..nl], 0) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    if len != dl {
        return err(EPERM);
    }
    // KMAP scratch fields the loader left: cairn_lba + sectors at +20/+24.
    let (clba, csec) = unsafe {
        let k = KMAP_RAM as *const u32;
        (k.add(5).read_volatile(), k.add(6).read_volatile())
    };
    // Re-ink the RAM copy first (volatile both ways — DMA-adjacent).
    let mut i = 0u64;
    while i < dl {
        let b = unsafe { ((buf + i) as *const u8).read_volatile() };
        unsafe { ((da + i) as *mut u8).write_volatile(b) };
        i += 1;
    }
    // Leaf xor field sits at the 4-aligned tail of the data.
    let xe = align_up(da - c.base + dl, 4);
    let mut x = 0u32;
    let mut q = 0u64;
    while q < align_up(dl, 4) {
        x ^= unsafe { ((da + q) as *const u32).read_volatile() };
        q += 4;
    }
    unsafe { ((c.base + xe) as *mut u32).write_volatile(x) };
    // Fresh cairn sum over the whole image, into the RAM KMAP scratch.
    let sum = clba ^ csec ^ cairn_xor(c.base, c.len) ^ CAIR_MAGIC;
    unsafe { ((KMAP_RAM + 28) as *mut u32).write_volatile(sum) };
    // Sectors touched: leaf bytes through the xor field (offsets, not addrs).
    let first = (da - c.base) / 512;
    let last = (xe + 4 - 1) / 512;
    let mut idx = first;
    while idx <= last {
        let ram = c.base + idx * 512;
        if !crate::ata::write_sectors(clba + idx as u32, 1, ram as *mut u8) {
            return err(EIO);
        }
        let scratch = core::ptr::addr_of_mut!(SECTOR_SCRATCH) as *mut u8;
        if !crate::ata::read_sectors(clba + idx as u32, 1, scratch) {
            return err(EIO);
        }
        if !sector_eq(ram, scratch as u64) {
            return err(EIO);
        }
        idx += 1;
    }
    // LBA0 carries the new sum — without it the next boot distrusts the cairn.
    if !crate::ata::write_sectors(0, 1, KMAP_RAM as *mut u8) {
        return err(EIO);
    }
    let scratch = core::ptr::addr_of_mut!(SECTOR_SCRATCH) as *mut u8;
    if !crate::ata::read_sectors(0, 1, scratch) {
        return err(EIO);
    }
    if !sector_eq(KMAP_RAM, scratch as u64) {
        return err(EIO);
    }
    dl
}

fn spawn_fail(n: u64) -> ! {
    serial_print("kindling: spawn FAIL ");
    serial_u64(n);
    serial_print("\n");
    hcf();
}

/// Copy a spark out of the cairn onto a contiguous pool run. None = scattered.
fn copy_out(src: u64, len: u64) -> Option<u64> {
    let pages = len.div_ceil(4096);
    let mut dst = 0u64;
    let mut prev = 0u64;
    let mut i = 0u64;
    while i < pages {
        let p = crate::mm::page_alloc();
        if dst == 0 {
            dst = p;
        } else if p != prev + 4096 {
            return None;
        }
        prev = p;
        let mut j = 0u64;
        while j < 4096 && i * 4096 + j < len {
            let b = unsafe { ((src + i * 4096 + j) as *const u8).read_volatile() };
            unsafe { ((p + j) as *mut u8).write_volatile(b) };
            j += 1;
        }
        i += 1;
    }
    Some(dst)
}

/// House call 5: named cairn spark onto private pages, own cup, own CR3.
/// Never returns on success. v0 replaces the light.
pub fn spawn(name_ptr: u64) -> u64 {
    let (name, nl) = match take_name(name_ptr) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let Some(c) = open() else {
        return err(ENODEV);
    };
    let (src, len) = match find(&c, &name[..nl], 1) {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    if len == 0 || len > 65536 {
        return err(EPERM);
    }
    let Some(dst) = copy_out(src, len) else {
        spawn_fail(4);
    };
    let realm = crate::mm::place_spark(dst, len);
    crate::house::enter_user_in(realm.spark, realm.cup_top, realm.cr3);
}

/// Loose `wick`: verify, copy, private CR3 + cup, enter at CPL3.
/// Never returns.
#[cfg(feature = "spawn-test")]
pub fn run_spawn(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let (src, len) = {
        let Some(c) = open() else {
            spawn_fail(1);
        };
        match find(&c, b"wick", 1) {
            Ok(v) => v,
            Err(_) => spawn_fail(2),
        }
    };
    if len == 0 || len > 65536 {
        spawn_fail(3);
    }
    let Some(dst) = copy_out(src, len) else {
        spawn_fail(4);
    };
    let realm = crate::mm::place_spark(dst, len);
    serial_print("kindling: spawn ok\n");
    crate::house::enter_user_in(realm.spark, realm.cup_top, realm.cr3);
}

/// Loose `ingle`: greeter spark, private CR3 + cup, waits for a key.
/// Never returns.
#[cfg(feature = "ingle-test")]
pub fn run_ingle(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let (src, len) = {
        let Some(c) = open() else {
            spawn_fail(1);
        };
        match find(&c, b"ingle", 1) {
            Ok(v) => v,
            Err(_) => spawn_fail(2),
        }
    };
    if len == 0 || len > 65536 {
        spawn_fail(3);
    }
    let Some(dst) = copy_out(src, len) else {
        spawn_fail(4);
    };
    let realm = crate::mm::place_spark(dst, len);
    serial_print("kindling: ingle ok\n");
    crate::house::enter_user_in(realm.spark, realm.cup_top, realm.cr3);
}

/// Paved path: light `ingle` when the cairn has it. Missing cairn or spark
/// writes `no ingle` on glass + serial — never a FAIL line on the happy kernel.
#[allow(dead_code)]
pub fn light_ingle(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    if let Some(c) = open() {
        if let Ok((src, len)) = find(&c, b"ingle", 1) {
            if len > 0 && len <= 65536 {
                if let Some(dst) = copy_out(src, len) {
                    let realm = crate::mm::place_spark(dst, len);
                    serial_print("kindling: ingle ok\n");
                    crate::house::enter_user_in(realm.spark, realm.cup_top, realm.cr3);
                }
            }
        }
    }
    serial_print("kindling: no ingle\n");
    let msg = b"no ingle\n";
    crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
    hcf();
}

/// Loose the `tale` spark: verify, copy it out of the cairn onto private
/// pool pages, point RSP0 home, enter at CPL3. The copy matters: the spark's
/// buf lives inside its own record, and running in place lets its writes
/// perturb the image the next boot verifies — the floor must be its own.
/// Never returns (the spark exits via the house gate).
#[cfg(feature = "tale-test")]
pub fn run_tale(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let (src, len) = {
        let Some(c) = open() else {
            serial_print("kindling: tale FAIL no-cairn\n");
            hcf();
        };
        match find(&c, b"tale", 1) {
            Ok(v) => v,
            Err(_) => {
                serial_print("kindling: tale FAIL no-spark\n");
                hcf();
            }
        }
    };
    // Sparks are small by law (64 KiB); the first pop off a fresh pool is a
    // contiguous run, asserted below — no silent scatter.
    if len == 0 || len > 65536 {
        serial_print("kindling: tale FAIL bad-spark\n");
        hcf();
    }
    let pages = len.div_ceil(4096);
    let mut dst = 0u64;
    let mut prev = 0u64;
    let mut i = 0u64;
    while i < pages {
        let p = crate::mm::page_alloc();
        if dst == 0 {
            dst = p;
        } else if p != prev + 4096 {
            serial_print("kindling: tale FAIL scattered\n");
            hcf();
        }
        prev = p;
        let mut j = 0u64;
        while j < 4096 && i * 4096 + j < len {
            let b = unsafe { ((src + i * 4096 + j) as *const u8).read_volatile() };
            unsafe { ((p + j) as *mut u8).write_volatile(b) };
            j += 1;
        }
        i += 1;
    }
    crate::house::enter_user(dst);
}
