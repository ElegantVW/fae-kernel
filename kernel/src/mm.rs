//! Phase 1 — Kindling's own well.
//! Identity map, bump allocator (mana), stack that is not the firmware thimble.

use core::arch::asm;

use crate::start::{serial_print, serial_u64};

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
    /// Loaded image start (EFI PE). 0 if the well already covers it.
    pub image_base: u64,
    /// GOP framebuffer physical address. 0 if none.
    pub fb_addr: u64,
    /// GOP framebuffer byte length (height * pitch).
    pub fb_len: u64,
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

/// Kernel CR3 after the well. Spawn clones this map and strips U/S.
static mut KERNEL_CR3: u64 = 0;

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
    let img_lo = hint.image_base;
    let img_hi = hint.kernel_end;
    let mut drink = hint.kernel_end.max(0x40_0000);
    let need = PAGE * 8 + STACK_MANA;
    if drink >= ram_end || ram_end.saturating_sub(drink) < need {
        // PE sits above the well cap (Insyde), or at the top of this bowl
        // (OVMF + extra USB). Prefer 4 MiB; sit after a low image only
        // when that still leaves a cup.
        drink = 0x40_0000;
        if img_lo != 0 && img_lo < ram_end && img_hi > drink {
            let after = align_up(img_hi, PAGE);
            if ram_end.saturating_sub(after) >= need {
                drink = after;
            }
        }
    }
    let need = drink + need;
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
    // Shared map stays all-U/S so tale/init still fetch the image. Spawn
    // clones these tables and strips US, leaving user bits only on the
    // spark and its cup.
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
    // MMIO window in PDPT[3] (≤3 GiB wells): 2M UC at 0xFEC00000 (IOAPIC
    // + HPET at 0xFED00000) and 0xFEE00000 (LAPIC). Supervisor-only.
    // Beyond 3 GiB the clock stays a stub — honestly degraded, not silent.
    let ap_mapped = unsafe {
        if (*pdpt)[3] == 0 {
            let ap_pd = bump.table();
            (*ap_pd)[502] = 0xFEC0_0000 | P | RW | PCD | PWT | PS;
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
    unsafe {
        core::ptr::addr_of_mut!(KERNEL_CR3).write(pml4 as u64);
    }
    // Still on firmware tables: plant the PE and GOP into the *new* map so
    // `mov cr3` does not unmap the running image or the glass.
    if img_lo != 0 && img_hi > img_lo {
        map_ident(img_lo, img_hi - img_lo, false);
    }
    if hint.fb_addr != 0 && hint.fb_len != 0 {
        map_ident(hint.fb_addr, hint.fb_len, true);
    }
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
// Reclaim proves the live-map split; spawn (G20) clones CR3 and runs.
#[cfg(feature = "reclaim-test")]
pub const REALM_PAGES: usize = 18; // guard + 16 cup + PT

pub(crate) fn page_alloc() -> u64 {
    unsafe {
        let n = core::ptr::addr_of_mut!(POOL_N);
        if n.read() == 0 {
            oom();
        }
        n.write(n.read() - 1);
        core::ptr::addr_of!(POOL).cast::<u64>().add(n.read()).read()
    }
}

// --- G20: spawn map. Clone the kernel tables, strip U/S from kernel
// leaves, split the 2M covering spark+cup+guard into 4K, and grant user
// only those floorboards. APIC PDPT[3] is shared supervisor.

const CUP_PAGES: u64 = 16;
const GUARD_AND_CUP: usize = 17;

pub struct SparkRealm {
    pub cr3: u64,
    pub spark: u64,
    pub cup_top: u64,
}

fn spawn_fail(n: u64) -> ! {
    serial_print("kindling: spawn FAIL ");
    serial_u64(n);
    serial_print("\n");
    crate::start::hcf();
}

fn copy_page(src: u64) -> u64 {
    let dst = page_alloc();
    unsafe {
        core::ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, PAGE as usize);
    }
    dst
}

fn clone_kernel_map() -> u64 {
    let kcr3 = unsafe { core::ptr::addr_of!(KERNEL_CR3).read() };
    if kcr3 == 0 {
        spawn_fail(5);
    }
    let pml4 = copy_page(kcr3);
    unsafe {
        let src_pml4 = kcr3 as *const u64;
        let dst_pml4 = pml4 as *mut u64;
        for i in 0..512 {
            let e = src_pml4.add(i).read_volatile();
            if e & P == 0 {
                dst_pml4.add(i).write_volatile(0);
                continue;
            }
            let src_pdpt_pa = pte_phys(e);
            let pdpt = copy_page(src_pdpt_pa);
            // US on the walk so CPL3 can reach user leaves.
            dst_pml4.add(i).write_volatile(pdpt | P | RW | US);
            let src_pdpt = src_pdpt_pa as *const u64;
            let dst_pdpt = pdpt as *mut u64;
            for gb in 0..512 {
                let pe = src_pdpt.add(gb).read_volatile();
                if pe & P == 0 {
                    dst_pdpt.add(gb).write_volatile(0);
                    continue;
                }
                // APIC window and any supervisor PDPT: share, no US.
                if gb >= 3 || pe & US == 0 {
                    dst_pdpt.add(gb).write_volatile(pe & !US);
                    continue;
                }
                if pe & PS != 0 {
                    dst_pdpt.add(gb).write_volatile(pe & !US);
                    continue;
                }
                let src_pd_pa = pte_phys(pe);
                let pd = copy_page(src_pd_pa);
                dst_pdpt.add(gb).write_volatile(pd | P | RW | US);
                let src_pd = src_pd_pa as *const u64;
                let dst_pd = pd as *mut u64;
                for j in 0..512 {
                    let le = src_pd.add(j).read_volatile();
                    if le & P == 0 {
                        dst_pd.add(j).write_volatile(0);
                    } else if le & PS != 0 {
                        dst_pd.add(j).write_volatile((le & !US) | P | RW | PS);
                    } else {
                        let pt = copy_page(pte_phys(le));
                        dst_pd.add(j).write_volatile(pt | P | RW | US);
                        let src_pt = pte_phys(le) as *const u64;
                        let dst_pt = pt as *mut u64;
                        for k in 0..512 {
                            let te = src_pt.add(k).read_volatile();
                            dst_pt
                                .add(k)
                                .write_volatile(if te & P != 0 { te & !US } else { 0 });
                        }
                    }
                }
            }
        }
    }
    pml4
}

fn leaf_entry(cr3: u64, va: u64) -> u64 {
    unsafe {
        let e0 = (cr3 as *const u64)
            .add(((va >> 39) & 511) as usize)
            .read_volatile();
        if e0 & P == 0 {
            return 0;
        }
        let e1 = (pte_phys(e0) as *const u64)
            .add(((va >> 30) & 511) as usize)
            .read_volatile();
        if e1 & P == 0 {
            return 0;
        }
        if e1 & PS != 0 {
            return e1;
        }
        let e2 = (pte_phys(e1) as *const u64)
            .add(((va >> 21) & 511) as usize)
            .read_volatile();
        if e2 & P == 0 {
            return 0;
        }
        if e2 & PS != 0 {
            return e2;
        }
        let e3 = (pte_phys(e2) as *const u64)
            .add(((va >> 12) & 511) as usize)
            .read_volatile();
        if e3 & P == 0 {
            return 0;
        }
        e3
    }
}

unsafe fn pte4k(cr3: u64, va: u64) -> *mut u64 {
    unsafe {
        let e0 = (cr3 as *const u64)
            .add(((va >> 39) & 511) as usize)
            .read_volatile();
        let e1 = (pte_phys(e0) as *const u64)
            .add(((va >> 30) & 511) as usize)
            .read_volatile();
        if e0 & P == 0 || e1 & P == 0 || e1 & PS != 0 {
            spawn_fail(6);
        }
        let pd = pte_phys(e1) as *mut u64;
        let pdi = ((va >> 21) & 511) as usize;
        let pde = pd.add(pdi).read_volatile();
        let pt = if pde & P == 0 {
            spawn_fail(6)
        } else if pde & PS != 0 {
            let pt = page_alloc();
            let region = va & !(BIG - 1);
            for i in 0..512 {
                let addr = region + (i as u64) * PAGE;
                (pt as *mut u64).add(i).write_volatile(addr | P | RW);
            }
            pd.add(pdi).write_volatile(pt | P | RW | US);
            pt
        } else {
            pte_phys(pde)
        };
        (pt as *mut u64).add(((va >> 12) & 511) as usize)
    }
}

unsafe fn grant_user(cr3: u64, va: u64) {
    unsafe {
        pte4k(cr3, va).write_volatile(va | P | RW | US);
    }
}

unsafe fn unmap_page(cr3: u64, va: u64) {
    unsafe {
        pte4k(cr3, va).write_volatile(0);
    }
}

/// Clone the kernel map, give `spark` and a 64 KiB cup U/S 4K leaves, shut
/// a guard page, keep kernel 2M supervisor. Verifies before return.
pub fn place_spark(spark: u64, spark_len: u64) -> SparkRealm {
    if spark_len == 0 || spark & (PAGE - 1) != 0 {
        spawn_fail(6);
    }
    let kcr3 = unsafe { core::ptr::addr_of!(KERNEL_CR3).read() };
    if kcr3 == 0 {
        spawn_fail(5);
    }
    let cr3 = clone_kernel_map();
    if cr3 == kcr3 {
        spawn_fail(5);
    }
    let np = spark_len.div_ceil(PAGE);
    let mut slot = [0u64; GUARD_AND_CUP];
    for i in 0..GUARD_AND_CUP {
        slot[i] = page_alloc();
        if i > 0 && slot[i] != slot[i - 1] + PAGE {
            spawn_fail(11);
        }
    }
    let guard = slot[0];
    let cup_base = slot[1];
    unsafe {
        (cup_base as *mut u64).write_volatile(CANARY);
        let mut p = 0u64;
        while p < np {
            grant_user(cr3, spark + p * PAGE);
            p += 1;
        }
        let mut c = 0u64;
        while c < CUP_PAGES {
            grant_user(cr3, cup_base + c * PAGE);
            c += 1;
        }
        unmap_page(cr3, guard);
    }
    let mut fail = 0u64;
    let mut p = 0u64;
    while p < np {
        let e = leaf_entry(cr3, spark + p * PAGE);
        if e & (P | US) != (P | US) || pte_phys(e) != spark + p * PAGE {
            fail = 6;
        }
        p += 1;
    }
    let ke = leaf_entry(cr3, 0x200000);
    if ke & (P | PS | US) != (P | PS) {
        fail = 7;
    }
    let mut c = 0u64;
    while c < CUP_PAGES {
        let e = leaf_entry(cr3, cup_base + c * PAGE);
        if e & (P | US) != (P | US) || pte_phys(e) != cup_base + c * PAGE {
            fail = 8;
        }
        c += 1;
    }
    if leaf_entry(cr3, guard) & P != 0 {
        fail = 9;
    }
    if !canary_ok(cup_base) {
        fail = 10;
    }
    if fail != 0 {
        spawn_fail(fail);
    }
    SparkRealm {
        cr3,
        spark,
        cup_top: (cup_base + CUP_PAGES * PAGE) & !0xF,
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

fn pte_phys(entry: u64) -> u64 {
    entry & 0x000F_FFFF_FFFF_F000
}

/// Present in the live CR3 (firmware tables, pre-well). Never touches the VA.
fn mapped(va: u64) -> bool {
    let mut cr3: u64;
    unsafe {
        asm!(
            "mov {}, cr3",
            out(reg) cr3,
            options(nomem, nostack, preserves_flags)
        );
    }
    let pml4 = pte_phys(cr3);
    let e0 = unsafe { ((pml4 + 8 * ((va >> 39) & 511)) as *const u64).read_volatile() };
    if e0 & 1 == 0 {
        return false;
    }
    let e1 = unsafe { ((pte_phys(e0) + 8 * ((va >> 30) & 511)) as *const u64).read_volatile() };
    if e1 & 1 == 0 {
        return false;
    }
    if e1 & PS != 0 {
        return true;
    }
    let e2 = unsafe { ((pte_phys(e1) + 8 * ((va >> 21) & 511)) as *const u64).read_volatile() };
    if e2 & 1 == 0 {
        return false;
    }
    if e2 & PS != 0 {
        return true;
    }
    let e3 = unsafe { ((pte_phys(e2) + 8 * ((va >> 12) & 511)) as *const u64).read_volatile() };
    e3 & 1 != 0
}

/// Walk 2 MiB steps with a write/read. Stop at CMOS cap, first not-present, or first lie.
/// Walking unmapped RAM is a #PF — that is a fail, not a well size.
fn probe_cap(cap: u64) -> u64 {
    let cap = cap.min(0x1_0000_0000);
    let mut last = 0x20_0000u64;
    let mut p = 0x40_0000u64;
    while p.saturating_add(8) <= cap {
        if !mapped(p) {
            break;
        }
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

/// Identity-map `phys..phys+len` as 2M uncacheable supervisor pages.
/// GOP / MMIO lives outside the well; without this, `mov cr3` blinds the glass.
/// Empty PDPT slots get a fresh PD. UC may replace a present 2M WB leaf (xHCI).
pub fn map_uc(phys: u64, len: u64) {
    map_ident(phys, len, true);
}

/// Identity-map 2M pages. `uc` sets PCD+PWT (GOP / xHCI). Present 2M leaves
/// stay unless `uc` (a BAR may sit on a well 2M page).
fn map_ident(phys: u64, len: u64, uc: bool) {
    if phys == 0 || len == 0 {
        return;
    }
    let start = phys & !(BIG - 1);
    let Some(sum) = phys.checked_add(len) else {
        return;
    };
    let end = align_up(sum, BIG);
    if end.saturating_sub(start) > 64 * 1024 * 1024 {
        return;
    }
    let cr3 = unsafe { core::ptr::addr_of!(KERNEL_CR3).read() };
    if cr3 == 0 {
        return;
    }
    let flags = if uc {
        P | RW | PCD | PWT | PS
    } else {
        P | RW | PS
    };
    unsafe {
        let pml4 = cr3 as *mut u64;
        let mut va = start;
        while va < end {
            let pml4i = ((va >> 39) & 511) as usize;
            let gb = ((va >> 30) & 511) as usize;
            let pdi = ((va >> 21) & 511) as usize;
            let pml4e = pml4.add(pml4i).read_volatile();
            let pdpt = if pml4e & P == 0 {
                let pdpt = page_alloc();
                core::ptr::write_bytes(pdpt as *mut u8, 0, PAGE as usize);
                pml4.add(pml4i).write_volatile(pdpt | P | RW);
                pdpt
            } else if pml4e & PS != 0 {
                va += BIG;
                continue;
            } else {
                pte_phys(pml4e)
            };
            let pdpt = pdpt as *mut u64;
            let pdpte = pdpt.add(gb).read_volatile();
            let pd = if pdpte & P == 0 {
                let pd = page_alloc();
                core::ptr::write_bytes(pd as *mut u8, 0, PAGE as usize);
                pdpt.add(gb).write_volatile(pd | P | RW);
                pd
            } else if pdpte & PS != 0 {
                va += BIG;
                continue;
            } else {
                pte_phys(pdpte)
            };
            let slot = (pd as *mut u64).add(pdi);
            let e = slot.read_volatile();
            if e & P == 0 {
                slot.write_volatile(va | flags);
            } else if uc && e & PS != 0 {
                // MMIO (xHCI) may land on a WB 2M RAM leaf. GOP already UC stays UC.
                slot.write_volatile(va | flags);
            }
            va += BIG;
        }
    }
    tlb_flush();
}
