//! Phase 1 — Kindling's own well.
//! Identity map, bump allocator (mana), stack that is not the firmware thimble.

use core::arch::asm;

use crate::start::serial_print;

pub const PAGE: u64 = 0x1000;
pub const BIG: u64 = 0x200000; // 2 MiB page
pub const STACK_MANA: u64 = 64 * 1024; // cup

const P: u64 = 1;
const RW: u64 = 2;
const US: u64 = 1 << 2;
const PWT: u64 = 1 << 3;
const PCD: u64 = 1 << 4;
const PS: u64 = 1 << 7;

pub struct Hint {
    /// First byte Kindling may drink (physical, inclusive).
    pub kernel_end: u64,
    /// First byte past RAM we dare map (physical, exclusive).
    pub ram_end: u64,
    /// EFI map is already the truth — do not probe (MMIO).
    pub trust_map: bool,
}

pub struct Bump {
    next: u64,
    end: u64,
}

impl Bump {
    fn new(start: u64, end: u64) -> Self {
        Self {
            next: align_up(start, PAGE),
            end,
        }
    }

    fn alloc(&mut self, size: u64, align: u64) -> *mut u8 {
        let n = align_up(self.next, align);
        let Some(new) = n.checked_add(size) else {
            oom();
        };
        if new > self.end {
            oom();
        }
        self.next = new;
        n as *mut u8
    }

    fn table(&mut self) -> *mut [u64; 512] {
        let p = self.alloc(PAGE, PAGE);
        unsafe {
            core::ptr::write_bytes(p, 0, PAGE as usize);
            p.cast()
        }
    }
}

fn align_up(x: u64, a: u64) -> u64 {
    (x + a - 1) & !(a - 1)
}

fn oom() -> ! {
    serial_print("kindling: the well ran dry\n");
    crate::start::hcf();
}

/// Reload CR3 with itself: full TLB flush (gate scale, no invlpg bookkeeping).
#[allow(dead_code)]
fn tlb_flush() {
    unsafe {
        let v: u64;
        asm!("mov {}, cr3", out(reg) v, options(nostack, preserves_flags));
        asm!("mov cr3, {}", in(reg) v, options(nostack, preserves_flags));
    }
}

// --- G1: frame pool. 4 KiB pages above the boot carve-out, LIFO discipline
// so create → drop → create hands back the same frames. Capped: the pool
// holds the lowest POOL_MAX pages; the rest stays mapped but unpooled
// (a bigger index is later work, not silent loss — pool_count tells).
const POOL_MAX: usize = 4096;

#[allow(dead_code)]
static mut POOL: [u64; POOL_MAX] = [0; POOL_MAX];
#[allow(dead_code)]
static mut POOL_N: usize = 0;

/// PD backing `PDPT[0]` (virt 0–1 GiB). Realms split one entry for a guard.
#[allow(dead_code)]
static mut PD0: u64 = 0;

/// True when the LAPIC window (PDPT[3]) was installed. The timer reads it.
#[allow(dead_code)]
static mut AP_MAPPED: bool = false;

/// Read by the timer (BIOS path) to decide virtual-wire vs honest stub.
#[allow(dead_code)]
pub fn ap_mapped() -> bool {
    unsafe { core::ptr::addr_of!(AP_MAPPED).read() }
}

fn pool_init(base: u64, end: u64) {
    let top = base.saturating_add((POOL_MAX as u64) * PAGE).min(end);
    let mut p = top;
    unsafe {
        while p > base {
            p -= PAGE;
            let n = core::ptr::addr_of_mut!(POOL_N);
            if n.read() < POOL_MAX {
                core::ptr::addr_of_mut!(POOL)
                    .cast::<u64>()
                    .add(n.read())
                    .write(p);
                n.write(n.read() + 1);
            } else {
                break;
            }
        }
    }
}

pub fn five_level() -> bool {
    let mut cr4: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    cr4 & (1 << 12) != 0
}

pub struct Well {
    pub top: u64,
    pub ram_end: u64,
    pub stack: u64,
    pub canary_at: u64,
    pub cr3: u64,
}

/// Build tables and a cup in RAM we already stand on. Caller switches RSP, then CR3.
pub unsafe fn prepare_well(hint: Hint) -> Well {
    let claimed = hint.ram_end.min(0x1_0000_0000);
    let ram_end = if hint.trust_map {
        claimed
    } else {
        probe_cap(claimed)
    } & !(BIG - 1);
    let drink = hint.kernel_end.max(0x40_0000);
    let need = drink + PAGE * 8 + STACK_MANA;
    if ram_end < need || ram_end <= drink {
        oom();
    }
    let mut bump = Bump::new(drink, ram_end);

    let pml4 = bump.table();
    let pdpt = bump.table();
    unsafe {
        (*pml4)[0] = pdpt as u64 | P | RW | US;
    }

    let mut virt = 0u64;
    let mut gb = 0usize;
    // Realms shut (init first): every page U/S so CPL3 can fetch the shared
    // image + stacks. Future realms clear US per-address-space.
    while virt < ram_end && gb < 512 {
        let pd = bump.table();
        unsafe {
            (*pdpt)[gb] = pd as u64 | P | RW | US;
            if gb == 0 {
                core::ptr::addr_of_mut!(PD0).write(pd as u64);
            }
            for i in 0..512 {
                if virt >= ram_end {
                    break;
                }
                (*pd)[i] = virt | P | RW | US | PS;
                virt += BIG;
            }
        }
        gb += 1;
    }

    let stack = bump.alloc(STACK_MANA, PAGE);
    let canary = CANARY;
    unsafe {
        stack.cast::<u64>().write_volatile(canary);
    }
    let top = ((stack as u64) + STACK_MANA) & !0xF;
    // LAPIC window for virtual-wire (BIOS timer path): one 2M UC page at
    // 0xFEE00000, supervisor-only (no US — CPL3 touching it faults, good).
    // Only when PDPT[3] is free (≤3 GiB; the gates never exceed 1 GiB).
    // Beyond 3 GiB the timer stays a stub — honestly degraded, not silent.
    let ap_mapped = unsafe {
        if (*pdpt)[3] == 0 {
            let ap_pd = bump.table();
            (*ap_pd)[503] = 0xFEE0_0000 | P | RW | PCD | PWT | PS;
            (*pdpt)[3] = ap_pd as u64 | P | RW;
            core::ptr::addr_of_mut!(AP_MAPPED).write(true);
            true
        } else {
            false
        }
    };
    let _ = ap_mapped;
    pool_init(align_up(bump.next, PAGE), ram_end);
    Well {
        top,
        ram_end,
        stack: STACK_MANA,
        canary_at: stack as u64,
        cr3: pml4 as u64,
    }
}

pub const CANARY: u64 = 0x00FA_E05F_AE05_FAE1;

pub fn canary_ok(base: u64) -> bool {
    unsafe { (base as *const u64).read_volatile() == CANARY }
}

pub fn well_mib(ram_end: u64) -> u64 {
    ram_end / (1024 * 1024)
}

pub fn cup_kib(stack: u64) -> u64 {
    stack / 1024
}

// --- G1: realms. A realm owns guard + 16 cup pages + 1 PT page from the
// pool and a split PD entry (2M → 512×4K) with the guard not-present.
// No execution on realm cups yet — G1 proves reclaim mechanics on the live
// map (split → verify → restore), execution waits on spawn (G4).
#[cfg(feature = "reclaim-test")]
pub const REALM_PAGES: usize = 18; // guard + 16 cup + PT

#[cfg(feature = "reclaim-test")]
fn page_alloc() -> u64 {
    unsafe {
        let n = core::ptr::addr_of_mut!(POOL_N);
        if n.read() == 0 {
            oom();
        }
        n.write(n.read() - 1);
        core::ptr::addr_of!(POOL).cast::<u64>().add(n.read()).read()
    }
}

#[cfg(feature = "reclaim-test")]
fn page_free(p: u64) {
    if p & (PAGE - 1) != 0 {
        oom();
    }
    unsafe {
        let n = core::ptr::addr_of_mut!(POOL_N);
        if n.read() >= POOL_MAX {
            oom();
        }
        core::ptr::addr_of_mut!(POOL)
            .cast::<u64>()
            .add(n.read())
            .write(p);
        n.write(n.read() + 1);
    }
}

#[cfg(feature = "reclaim-test")]
fn pool_count() -> usize {
    unsafe { core::ptr::addr_of!(POOL_N).read() }
}

#[cfg(feature = "reclaim-test")]
pub struct Realm {
    guard: u64,
    cup_base: u64,
    pt: u64,
    pages: [u64; REALM_PAGES],
    pd_index: usize,
    saved_pd: u64,
}

#[cfg(feature = "reclaim-test")]
impl Realm {
    /// Carve guard + cup + PT, split the covering 2M PD entry, flush.
    pub unsafe fn create() -> Realm {
        unsafe {
            let mut pages = [0u64; REALM_PAGES];
            for slot in pages.iter_mut() {
                *slot = page_alloc();
            }
            // LIFO discipline: pops ascend, so the run must be contiguous.
            for i in 0..REALM_PAGES - 1 {
                if pages[i] + PAGE != pages[i + 1] {
                    oom();
                }
            }
            let guard = pages[0];
            let cup_base = pages[1];
            let pt = pages[REALM_PAGES - 1];
            if cup_base >= (1 << 30) {
                oom(); // G1 scope: low realms (PDPT[0]) only.
            }
            (cup_base as *mut u64).write_volatile(CANARY);
            let pd_index = ((cup_base >> 21) & 511) as usize;
            let pd = core::ptr::addr_of!(PD0).read() as *mut u64;
            let saved_pd = pd.add(pd_index).read_volatile();
            if saved_pd & (P | PS) != (P | PS) {
                oom(); // already split — never halve a halve.
            }
            let region = (pd_index as u64) << 21;
            let gi = ((guard - region) / PAGE) as usize;
            for i in 0..512 {
                let addr = region + (i as u64) * PAGE;
                let e = if i == gi { 0 } else { addr | P | RW | US };
                (pt as *mut u64).add(i).write_volatile(e);
            }
            pd.add(pd_index).write_volatile(pt | P | RW | US);
            tlb_flush();
            Realm {
                guard,
                cup_base,
                pt,
                pages,
                pd_index,
                saved_pd,
            }
        }
    }

    fn guard_closed(&self) -> bool {
        let region = (self.pd_index as u64) << 21;
        let gi = ((self.guard - region) / PAGE) as usize;
        unsafe { (self.pt as *const u64).add(gi).read_volatile() & P == 0 }
    }

    /// Verify canary, restore the 2M entry, flush, hand back all pages
    /// in exact reverse order so the next create replays the same frames.
    pub unsafe fn drop(self) {
        unsafe {
            if !canary_ok(self.cup_base) {
                serial_print("kindling: the cup was bitten\n");
                crate::start::hcf();
            }
            let pd = core::ptr::addr_of!(PD0).read() as *mut u64;
            pd.add(self.pd_index).write_volatile(self.saved_pd);
            tlb_flush();
            for i in (0..REALM_PAGES).rev() {
                page_free(self.pages[i]);
            }
        }
    }
}

/// Ring-0 reclaim proof: create → guard shut → drop → free count whole →
/// create replays the same frames → guard shut → drop → count whole.
#[cfg(feature = "reclaim-test")]
pub fn reclaim_self_test() {
    use crate::start::{hcf, serial_u64};

    let c0 = pool_count();
    let mut fail = 0u64;
    unsafe {
        let a = Realm::create();
        if !a.guard_closed() {
            fail = 1;
        }
        let (cup_a, pt_a) = (a.cup_base, a.pt);
        a.drop();
        if pool_count() != c0 {
            fail = 2;
        }
        let b = Realm::create();
        if b.cup_base != cup_a || b.pt != pt_a {
            fail = 3;
        }
        if !b.guard_closed() {
            fail = 4;
        }
        b.drop();
        if pool_count() != c0 {
            fail = 5;
        }
    }
    if fail != 0 {
        serial_print("kindling: reclaim FAIL ");
        serial_u64(fail);
        serial_print("\n");
        hcf();
    }
    serial_print("kindling: reclaim ok\n");
}

/// Walk 2 MiB steps with a write/read. Stop at CMOS cap or first lie.
fn probe_cap(cap: u64) -> u64 {
    let cap = cap.min(0x1_0000_0000);
    let mut last = 0x20_0000u64;
    let mut p = 0x40_0000u64;
    while p.saturating_add(8) <= cap {
        let ptr = p as *mut u64;
        let token = 0x4B4E_444C_u64 ^ p;
        let old = unsafe { ptr.read_volatile() };
        unsafe { ptr.write_volatile(token) };
        let got = unsafe { ptr.read_volatile() };
        unsafe { ptr.write_volatile(old) };
        if got != token {
            break;
        }
        last = p.saturating_add(BIG);
        p = p.saturating_add(BIG);
    }
    last.min(cap)
}
