//! Phase 1 — Kindling's own well.
//! Identity map, bump allocator (mana), stack that is not the firmware thimble.

use core::arch::asm;

use crate::start::serial_print;

pub const PAGE: u64 = 0x1000;
pub const BIG: u64 = 0x200000; // 2 MiB page
pub const STACK_MANA: u64 = 64 * 1024; // cup

const P: u64 = 1;
const RW: u64 = 2;
const PS: u64 = 1 << 7;

pub struct Hint {
    /// First byte Kindling may drink (physical, inclusive).
    pub kernel_end: u64,
    /// First byte past RAM we dare map (physical, exclusive).
    pub ram_end: u64,
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

/// Identity-map 0..ram_end with 2 MiB pages, load CR3, return new stack top.
pub unsafe fn take_the_well(hint: Hint) -> (*mut u8, u64, u64, u64) {
    let ram_end = hint.ram_end.min(0x1_0000_0000) & !(BIG - 1);
    let drink = hint.kernel_end.max(0x40_0000);
    let need = drink + PAGE * 8 + STACK_MANA;
    if ram_end < need || ram_end <= drink {
        oom();
    }
    let mut bump = Bump::new(drink, ram_end);

    let pml4 = bump.table();
    let pdpt = bump.table();
    unsafe {
        (*pml4)[0] = pdpt as u64 | P | RW;
    }

    let mut virt = 0u64;
    let mut gb = 0usize;
    while virt < ram_end && gb < 512 {
        let pd = bump.table();
        unsafe {
            (*pdpt)[gb] = pd as u64 | P | RW;
            for i in 0..512 {
                if virt >= ram_end {
                    break;
                }
                (*pd)[i] = virt | P | RW | PS;
                virt += BIG;
            }
        }
        gb += 1;
    }

    unsafe {
        asm!("mov cr3, {}", in(reg) pml4 as u64, options(nostack));
    }

    let stack = bump.alloc(STACK_MANA, PAGE);
    let canary = CANARY;
    unsafe {
        stack.cast::<u64>().write_volatile(canary);
    }
    let top = ((stack as u64) + STACK_MANA) & !0xF;
    (top as *mut u8, ram_end, STACK_MANA, stack as u64)
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
