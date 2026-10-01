//! Cairn — leaves + sparks at 0x100000, laid by the loader (docs/CAIRN.md).
//! The kernel re-verifies everything: KMAP scratch magic, cairn checksum,
//! header, walk bounds, per-leaf xor. Absent or corrupt is refusal, never
//! partial bytes. v0 rides the paved path only — EFI/Limine hands have no
//! cairn and `glean` there refuses `-ENODEV`, loudly.

#[cfg(feature = "tale-test")]
use crate::start::{hcf, serial_print};

pub const GLEAN: u64 = 8;
pub const STOW: u64 = 9; // shut until the G2b write path

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

/// Locate + verify the cairn via the KMAP scratch the loader left at 0x8400.
fn open() -> Option<Cairn> {
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

/// Gather a leaf's bytes: `rdi` = NUL-terminated name (≤64, plain ascii, no
/// `/`), `rsi` = buf, `rdx` = len (cap 1 MiB). Returns bytes copied (a short
/// read when the leaf is longer is honest, not an error) or `-errno`.
/// v0 trusts mapped RAM for pointers (shared all-U/S map); realms (G4) will
/// check callers properly.
pub fn glean(name_ptr: u64, buf: u64, len: u64) -> u64 {
    if name_ptr == 0 || buf == 0 || len == 0 || len > MAX_DATA {
        return err(EPERM);
    }
    let mut name = [0u8; 64];
    let mut nl = 0usize;
    loop {
        if nl >= 65 {
            return err(EPERM);
        }
        let b = unsafe { ((name_ptr + nl as u64) as *const u8).read_volatile() };
        if b == 0 {
            break;
        }
        if nl >= 64 || b == b'/' || !(0x20..=0x7E).contains(&b) {
            return err(EPERM);
        }
        name[nl] = b;
        nl += 1;
    }
    if nl == 0 {
        return err(EPERM);
    }
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

/// Loose the `tale` spark: verify, point RSP0 home, enter at CPL3.
/// Never returns (the spark exits via the house gate).
#[cfg(feature = "tale-test")]
pub fn run_tale(cup_top: u64) -> ! {
    crate::gdt::set_kernel_stack(cup_top);
    let entry = {
        let Some(c) = open() else {
            serial_print("kindling: tale FAIL no-cairn\n");
            hcf();
        };
        match find(&c, b"tale", 1) {
            Ok((da, dl)) => {
                if dl == 0 || dl > MAX_DATA {
                    serial_print("kindling: tale FAIL bad-spark\n");
                    hcf();
                }
                da
            }
            Err(_) => {
                serial_print("kindling: tale FAIL no-spark\n");
                hcf();
            }
        }
    };
    crate::house::enter_user(entry);
}
