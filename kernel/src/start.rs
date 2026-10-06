use core::arch::asm;
use core::fmt::{self, Write};

use crate::mm::{self, Hint};

const COM1: u16 = 0x3F8;
// mana: firmware handed us a thimble at 0x7000. take_the_well pours a cup.

#[inline]
unsafe fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags))
    }
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe {
        asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    val
}

pub fn serial_init() {
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1, 0x03);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xC7);
        outb(COM1 + 4, 0x0B);
    }
    // 16-color slot 13 Lilac, in case firmware did not already cast it.
    serial_print("\x1b[95m");
}

// IdeaPad has no 0x3F8. If LSR never says THR empty, one unbounded wait
// is a silent hang after ExitBootServices (ConOut already dead).
static mut SERIAL_LIVE: bool = true;
const SERIAL_SPINS: u32 = 100_000;

fn serial_put(b: u8) {
    unsafe {
        if !core::ptr::addr_of!(SERIAL_LIVE).read() {
            return;
        }
        let mut n = 0u32;
        while inb(COM1 + 5) & 0x20 == 0 {
            n = n.saturating_add(1);
            if n >= SERIAL_SPINS {
                core::ptr::addr_of_mut!(SERIAL_LIVE).write(false);
                return;
            }
            core::hint::spin_loop();
        }
        outb(COM1, b);
    }
}

pub(crate) fn serial_put_byte(b: u8) {
    if b == b'\n' {
        serial_put(b'\r');
    }
    serial_put(b);
}

pub struct Serial;

impl Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                serial_put(b'\r');
            }
            serial_put(b);
        }
        Ok(())
    }
}

pub fn serial_print(s: &str) {
    let _ = Serial.write_str(s);
}

pub(crate) fn serial_u64(n: u64) {
    if n == 0 {
        serial_print("0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 20;
    let mut x = n;
    while x > 0 {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    let _ = Serial.write_str(core::str::from_utf8(&buf[i..]).unwrap());
}

pub(crate) fn serial_hex(n: u64) {
    let mut buf = [0u8; 16];
    let mut i = 0usize;
    while i < 16 {
        let d = ((n >> (60 - 4 * i)) & 0xF) as u8;
        buf[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
        i += 1;
    }
    let _ = Serial.write_str(core::str::from_utf8(&buf).unwrap());
}

/// Ceremony first. Then, if `hint` is Some, Kindling takes the well and a cup-stack.
pub fn start(hint: Option<Hint>) -> ! {
    serial_init();
    serial_print("fae-kernel\n");
    serial_print("kindling: still only a spark\n");
    crate::cpu::init_pic();
    if matches!(&hint, Some(h) if !h.trust_map) {
        crate::glass::offer_vga();
    }
    crate::gdt::install();
    crate::idt::install();
    if matches!(&hint, Some(h) if !h.trust_map) {
        crate::timer::init();
    }
    crate::kbd::init();
    crate::cpu::enable_fpu_sse();
    #[cfg(feature = "trap6")]
    unsafe {
        asm!("ud2");
    }
    if let Some(hint) = hint {
        if mm::five_level() {
            serial_print("kindling: well waits (5-level)\n");
            hcf();
        }
        if hint.trust_map {
            unsafe {
                core::ptr::addr_of_mut!(FROM_EFI).write(true);
            }
            serial_print("kindling: efi map ");
            serial_u64(hint.ram_end / (1024 * 1024));
            serial_print(" MiB\n");
        }
        let well = unsafe { mm::prepare_well(hint) };
        unsafe {
            core::ptr::addr_of_mut!(WELL_MIB).write(mm::well_mib(well.ram_end));
            core::ptr::addr_of_mut!(CUP_KIB).write(mm::cup_kib(well.stack));
            core::ptr::addr_of_mut!(CANARY_AT).write(well.canary_at);
            core::ptr::addr_of_mut!(CUP_TOP).write(well.top);
            asm!(
                "mov rsp, {top}",
                "mov cr3, {cr3}",
                "jmp {cont}",
                top = in(reg) well.top,
                cr3 = in(reg) well.cr3,
                cont = sym after_cup,
                options(noreturn)
            );
        }
    }
    // Limine crutch: no well, firmware tables still hold the GOP.
    crate::glass::show();
    hcf();
}

static mut WELL_MIB: u64 = 0;
static mut CUP_KIB: u64 = 0;
static mut CANARY_AT: u64 = 0;
static mut CUP_TOP: u64 = 0;
static mut FROM_EFI: bool = false;

#[inline(never)]
unsafe extern "C" fn after_cup() -> ! {
    if !mm::canary_ok(unsafe { core::ptr::addr_of!(CANARY_AT).read() }) {
        serial_print("kindling: the cup was bitten\n");
        hcf();
    }
    // Post-well, on our own tables: wire virtual-wire before any test that
    // reads it. Unconditional — harmless on images that never sleep.
    crate::timer::wire();
    crate::timer::arm();
    crate::glass::map_and_show();
    if let Some((addr, pitch, _, _)) = crate::glass::fb_info() {
        serial_print("kindling: gop 0x");
        serial_hex(addr);
        serial_print(" 0x");
        serial_hex(pitch);
        serial_print("\n");
    }
    {
        let msg = b"well\n";
        crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
        crate::usb::init();
        if crate::usb::found() {
            let msg = b"xhci\n";
            crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
            if crate::usb::kbd_live() {
                serial_print("kindling: usb kbd\n");
                let msg = b"usb kbd\n";
                crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
            } else {
                serial_print("kindling: no usb\n");
                let msg = b"no usb\n";
                crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
            }
        } else {
            serial_print("kindling: no xhci\n");
            let msg = b"no xhci\n";
            crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
        }
        if !crate::kbd::live() && !crate::usb::kbd_live() {
            let msg = b"no kbd\n";
            crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
        }
        if crate::timer::clock_live() {
            serial_print("kindling: tick\n");
            let msg = b"tick\n";
            crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
        } else {
            serial_print("kindling: no tick\n");
            let msg = b"no tick\n";
            crate::glass::put_bytes(msg.as_ptr(), msg.len() as u64);
        }
        if unsafe { core::ptr::addr_of!(FROM_EFI).read() } && crate::timer::clock_live() {
            let t0 = crate::timer::ms();
            crate::timer::sleep_ms(50);
            let t1 = crate::timer::ms();
            if t1.saturating_sub(t0) >= 40 {
                serial_print("kindling: efi slept\n");
            } else {
                serial_print("kindling: efi sleep short\n");
            }
        }
        {
            let line = crate::usb::msc_line();
            serial_print("kindling: ");
            serial_print(line);
            serial_print("\n");
            crate::glass::put_bytes(line.as_ptr(), line.len() as u64);
            crate::glass::put_bytes(b"\n".as_ptr(), 1);
        }
        if crate::usb::msc_live() {
            let line = crate::usb::fat_line();
            serial_print("kindling: ");
            serial_print(line);
            serial_print("\n");
            crate::glass::put_bytes(line.as_ptr(), line.len() as u64);
            crate::glass::put_bytes(b"\n".as_ptr(), 1);
        }
    }
    #[cfg(feature = "house-test")]
    crate::house::self_test();
    #[cfg(feature = "reclaim-test")]
    crate::mm::reclaim_self_test();
    serial_print("kindling: well ");
    serial_u64(unsafe { core::ptr::addr_of!(WELL_MIB).read() });
    serial_print(" MiB · stack ");
    serial_u64(unsafe { core::ptr::addr_of!(CUP_KIB).read() });
    serial_print(" KiB cup\n");
    #[cfg(feature = "ingle-test")]
    crate::cairn::run_ingle(unsafe { core::ptr::addr_of!(CUP_TOP).read() });
    #[cfg(all(feature = "spawn-test", not(feature = "ingle-test")))]
    crate::cairn::run_spawn(unsafe { core::ptr::addr_of!(CUP_TOP).read() });
    #[cfg(all(
        feature = "tale-test",
        not(feature = "spawn-test"),
        not(feature = "ingle-test")
    ))]
    crate::cairn::run_tale(unsafe { core::ptr::addr_of!(CUP_TOP).read() });
    #[cfg(all(
        feature = "ring3-test",
        not(feature = "tale-test"),
        not(feature = "spawn-test"),
        not(feature = "ingle-test")
    ))]
    crate::house::enter_init(unsafe { core::ptr::addr_of!(CUP_TOP).read() });
    #[cfg(not(any(
        feature = "ring3-test",
        feature = "tale-test",
        feature = "spawn-test",
        feature = "ingle-test"
    )))]
    crate::cairn::light_ingle(unsafe { core::ptr::addr_of!(CUP_TOP).read() });
}

pub fn hcf() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
