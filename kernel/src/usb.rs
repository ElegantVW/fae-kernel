//! xHCI + HID boot keyboard. Event ring is polled; house `read` drains it.

use core::cell::UnsafeCell;
use core::sync::atomic::{compiler_fence, Ordering};

use crate::start::{serial_hex, serial_print};

const PAGE: usize = 4096;
const SPINS: u32 = 4_000_000;
const RING: u32 = 256;

const TRB_NORMAL: u32 = 1;
const TRB_SETUP: u32 = 2;
const TRB_DATA: u32 = 3;
const TRB_STATUS: u32 = 4;
const TRB_LINK: u32 = 6;
const TRB_ENABLE_SLOT: u32 = 9;
const TRB_ADDRESS: u32 = 11;
const TRB_CONFIG_EP: u32 = 12;
const TRB_XFER: u32 = 32;
const TRB_CMD: u32 = 33;
const TRB_PORT: u32 = 34;

const IOC: u32 = 1 << 5;
const IDT: u32 = 1 << 6;
const CH: u32 = 1 << 4;
const ISP: u32 = 1 << 2;
const TC: u32 = 1 << 1;

const RS: u32 = 1;
const HCRST: u32 = 2;
const HCH: u32 = 1;
const CNR: u32 = 1 << 11;
const EINT: u32 = 1 << 3;

const CCS: u32 = 1;
const PED: u32 = 2;
const PR: u32 = 1 << 4;
const PP: u32 = 1 << 9;
const CSC: u32 = 1 << 17;
const PEC: u32 = 1 << 18;
const WRC: u32 = 1 << 19;
const OCC: u32 = 1 << 20;
const PRC: u32 = 1 << 21;
const PLC: u32 = 1 << 22;
const CEC: u32 = 1 << 23;
const WPR: u32 = 1 << 31;
const PORT_CHG: u32 = CSC | PEC | WRC | OCC | PRC | PLC | CEC;

const CC_OK: u32 = 1;
const CC_SHORT: u32 = 13;

struct Ring {
    base: u64,
    i: u32,
    c: u32,
}

struct Evt {
    base: u64,
    i: u32,
    c: u32,
}

struct Host {
    op: u64,
    rt: u64,
    db: u64,
    ctxsz: u32,
    max_ports: u32,
    ppc: bool,
    cmd: Ring,
    evt: Evt,
    dcbaa: u64,
    in_ctx: u64,
    out_ctx: u64,
    ep0: Ring,
    intr: Ring,
    data: u64,
    report: u64,
    cmd_seen: bool,
    cmd_code: u32,
    cmd_slot: u32,
    xfer_seen: bool,
    xfer_code: u32,
    kbd_slot: u8,
    kbd_dci: u8,
    kbd_prev: [u8; 6],
}

struct HostCell(UnsafeCell<Option<Host>>);
unsafe impl Sync for HostCell {}

static HOST: HostCell = HostCell(UnsafeCell::new(None));
static mut KBD: bool = false;

fn host_mut() -> &'static mut Option<Host> {
    unsafe { &mut *HOST.0.get() }
}

fn r8(a: u64) -> u8 {
    unsafe { (a as *const u8).read_volatile() }
}
fn r32(a: u64) -> u32 {
    unsafe { (a as *const u32).read_volatile() }
}
fn w32(a: u64, v: u32) {
    unsafe { (a as *mut u32).write_volatile(v) }
}
fn r64(a: u64) -> u64 {
    unsafe { (a as *const u64).read_volatile() }
}
fn w64(a: u64, v: u64) {
    unsafe { (a as *mut u64).write_volatile(v) }
}

fn wr_trb(addr: u64, param: u64, status: u32, ctrl: u32) {
    unsafe {
        (addr as *mut u64).write_volatile(param);
        (addr as *mut u32).add(2).write_volatile(status);
    }
    compiler_fence(Ordering::Release);
    unsafe {
        (addr as *mut u32).add(3).write_volatile(ctrl);
    }
}

fn rd_trb(addr: u64) -> (u64, u32, u32) {
    let ctrl = r32(addr + 12);
    let param = r64(addr);
    let status = r32(addr + 8);
    (param, status, ctrl)
}

fn page() -> u64 {
    let p = crate::mm::page_alloc();
    unsafe {
        core::ptr::write_bytes(p as *mut u8, 0, PAGE);
    }
    p
}

fn wait_eq(addr: u64, mask: u32, want: u32) -> bool {
    let mut n = 0u32;
    while n < SPINS {
        if r32(addr) & mask == want {
            return true;
        }
        n = n.saturating_add(1);
        core::hint::spin_loop();
    }
    false
}

fn wait_set(addr: u64, mask: u32) -> bool {
    let mut n = 0u32;
    while n < SPINS {
        if r32(addr) & mask != 0 {
            return true;
        }
        n = n.saturating_add(1);
        core::hint::spin_loop();
    }
    false
}

fn ring_new(base: u64) -> Ring {
    wr_trb(
        base + ((RING as u64) - 1) * 16,
        base,
        0,
        (TRB_LINK << 10) | TC | 1,
    );
    Ring { base, i: 0, c: 1 }
}

impl Ring {
    fn enq(&mut self, param: u64, status: u32, ctrl: u32) -> u64 {
        let addr = self.base + self.i as u64 * 16;
        wr_trb(addr, param, status, ctrl | self.c);
        self.i = self.i.saturating_add(1);
        if self.i == RING - 1 {
            wr_trb(
                self.base + (RING as u64 - 1) * 16,
                self.base,
                0,
                (TRB_LINK << 10) | TC | self.c,
            );
            self.i = 0;
            self.c ^= 1;
        }
        addr
    }
}

fn portsc(op: u64, port: u32) -> u64 {
    op + 0x400 + (port as u64 - 1) * 0x10
}

fn doorbell(h: &Host, slot: u32, target: u32) {
    compiler_fence(Ordering::SeqCst);
    w32(h.db + slot as u64 * 4, target);
}

fn process_events(h: &mut Host) {
    loop {
        let addr = h.evt.base + h.evt.i as u64 * 16;
        let (param, status, ctrl) = rd_trb(addr);
        if (ctrl & 1) != h.evt.c {
            break;
        }
        let ty = (ctrl >> 10) & 0x3F;
        match ty {
            TRB_CMD => {
                h.cmd_seen = true;
                h.cmd_code = status >> 24;
                h.cmd_slot = ctrl >> 24;
            }
            TRB_XFER => {
                h.xfer_seen = true;
                h.xfer_code = status >> 24;
                let slot = (ctrl >> 24) as u8;
                let epid = ((ctrl >> 16) & 0x1F) as u8;
                if h.kbd_slot != 0 && slot == h.kbd_slot && epid == h.kbd_dci {
                    hid_report(h);
                }
            }
            TRB_PORT => {
                let port = ((param >> 24) & 0xFF) as u32;
                if (1..256).contains(&port) {
                    let a = portsc(h.op, port);
                    w32(a, r32(a) | PORT_CHG);
                }
            }
            _ => {}
        }
        h.evt.i = h.evt.i.saturating_add(1);
        if h.evt.i == RING {
            h.evt.i = 0;
            h.evt.c ^= 1;
        }
        let erdp = h.evt.base + h.evt.i as u64 * 16;
        w64(h.rt + 0x38, erdp | (1 << 3));
    }
    w32(h.op + 4, EINT);
}

fn hid_report(h: &mut Host) {
    let mut now = [0u8; 6];
    unsafe {
        let p = h.report as *const u8;
        let mut i = 0usize;
        while i < 6 {
            now[i] = p.add(2 + i).read_volatile();
            i += 1;
        }
    }
    let mut i = 0usize;
    while i < 6 {
        let k = now[i];
        if k != 0 {
            let mut was = false;
            let mut j = 0usize;
            while j < 6 {
                if h.kbd_prev[j] == k {
                    was = true;
                    break;
                }
                j += 1;
            }
            if !was {
                let a = hid_ascii(k);
                if a != 0 {
                    crate::kbd::push_ascii(a);
                }
            }
        }
        i += 1;
    }
    h.kbd_prev = now;
    let _ = h.intr.enq(h.report, 8, (TRB_NORMAL << 10) | IOC | ISP);
    doorbell(h, h.kbd_slot as u32, h.kbd_dci as u32);
}

fn hid_ascii(u: u8) -> u8 {
    match u {
        0x04..=0x1D => b'a' + (u - 0x04),
        0x1E => b'1',
        0x1F => b'2',
        0x20 => b'3',
        0x21 => b'4',
        0x22 => b'5',
        0x23 => b'6',
        0x24 => b'7',
        0x25 => b'8',
        0x26 => b'9',
        0x27 => b'0',
        0x28 => b'\n',
        0x2A => 0x08,
        0x2C => b' ',
        _ => 0,
    }
}

fn command(h: &mut Host, param: u64, status: u32, ctrl: u32) -> bool {
    h.cmd_seen = false;
    let _ = h.cmd.enq(param, status, ctrl);
    doorbell(h, 0, 0);
    let mut n = 0u32;
    while n < SPINS {
        process_events(h);
        if h.cmd_seen {
            return h.cmd_code == CC_OK;
        }
        n = n.saturating_add(1);
        core::hint::spin_loop();
    }
    false
}

fn enable_slot(h: &mut Host) -> Option<u8> {
    if !command(h, 0, 0, TRB_ENABLE_SLOT << 10) {
        return None;
    }
    let s = h.cmd_slot as u8;
    if s == 0 {
        None
    } else {
        Some(s)
    }
}

fn address_device(h: &mut Host, slot: u8, port: u32, speed: u32) -> bool {
    unsafe {
        core::ptr::write_bytes(h.in_ctx as *mut u8, 0, PAGE);
        core::ptr::write_bytes(h.out_ctx as *mut u8, 0, PAGE);
        core::ptr::write_bytes(h.ep0.base as *mut u8, 0, PAGE);
    }
    h.ep0 = ring_new(h.ep0.base);
    let cs = h.ctxsz as u64;
    // Input control: A0 | A1
    w32(h.in_ctx + 4, 0b11);
    let slot_ctx = h.in_ctx + cs;
    w32(slot_ctx, (1 << 27) | (speed << 20));
    w32(slot_ctx + 4, port << 16);
    let ep0 = h.in_ctx + 2 * cs;
    let mps: u32 = match speed {
        2 => 8,
        4 | 5 => 512,
        _ => 64,
    };
    w32(ep0 + 4, (mps << 16) | (4 << 3) | (3 << 1));
    w64(ep0 + 8, h.ep0.base | h.ep0.c as u64);
    w64(h.dcbaa + slot as u64 * 8, h.out_ctx);
    command(
        h,
        h.in_ctx,
        0,
        (TRB_ADDRESS << 10) | ((slot as u32) << 24),
    )
}

fn control(h: &mut Host, slot: u8, setup: [u8; 8], buf: u64, len: u16, din: bool) -> bool {
    let pkt = u64::from_le_bytes(setup);
    let trt: u32 = if len == 0 {
        0
    } else if din {
        2
    } else {
        3
    };
    let _ = h.ep0.enq(pkt, 8, (TRB_SETUP << 10) | IDT | CH | (trt << 16));
    if len != 0 {
        let dir = if din { 1u32 << 16 } else { 0 };
        let _ = h.ep0.enq(buf, len as u32, (TRB_DATA << 10) | CH | dir);
    }
    let sdir = if din && len != 0 { 0 } else { 1u32 << 16 };
    let _ = h.ep0.enq(0, 0, (TRB_STATUS << 10) | IOC | sdir);
    h.xfer_seen = false;
    doorbell(h, slot as u32, 1);
    let mut n = 0u32;
    while n < SPINS {
        process_events(h);
        if h.xfer_seen {
            return h.xfer_code == CC_OK || h.xfer_code == CC_SHORT;
        }
        n = n.saturating_add(1);
        core::hint::spin_loop();
    }
    false
}

fn get_desc(h: &mut Host, slot: u8, ty: u8, idx: u8, len: u16) -> bool {
    let mut s = [0u8; 8];
    s[0] = 0x80;
    s[1] = 6;
    s[2] = idx;
    s[3] = ty;
    s[6] = (len & 0xFF) as u8;
    s[7] = (len >> 8) as u8;
    unsafe {
        core::ptr::write_bytes(h.data as *mut u8, 0, PAGE);
    }
    control(h, slot, s, h.data, len, true)
}

fn set_config(h: &mut Host, slot: u8, cfg: u8) -> bool {
    let mut s = [0u8; 8];
    s[1] = 9;
    s[2] = cfg;
    control(h, slot, s, 0, 0, false)
}

fn hid_set(h: &mut Host, slot: u8, iface: u8, req: u8, value: u16) -> bool {
    let mut s = [0u8; 8];
    s[0] = 0x21;
    s[1] = req;
    s[2] = (value & 0xFF) as u8;
    s[3] = (value >> 8) as u8;
    s[4] = iface;
    control(h, slot, s, 0, 0, false)
}

fn ep_interval(speed: u32, binterval: u8) -> u32 {
    if speed >= 3 {
        binterval.saturating_sub(1).min(15) as u32
    } else {
        let mut v = binterval.max(1);
        let mut i = 0u32;
        while v > 1 && i < 15 {
            v >>= 1;
            i += 1;
        }
        i
    }
}

fn config_intr(
    h: &mut Host,
    slot: u8,
    port: u32,
    speed: u32,
    ep_addr: u8,
    maxpkt: u16,
    binterval: u8,
) -> bool {
    let epnum = ep_addr & 0x0F;
    let din = ep_addr & 0x80 != 0;
    let dci = (epnum as u32) * 2 + if din { 1 } else { 0 };
    unsafe {
        core::ptr::write_bytes(h.in_ctx as *mut u8, 0, PAGE);
        core::ptr::write_bytes(h.intr.base as *mut u8, 0, PAGE);
    }
    h.intr = ring_new(h.intr.base);
    let cs = h.ctxsz as u64;
    w32(h.in_ctx + 4, (1 << dci) | 1);
    let slot_ctx = h.in_ctx + cs;
    w32(slot_ctx, (dci << 27) | (speed << 20));
    w32(slot_ctx + 4, port << 16);
    let ep = h.in_ctx + (dci as u64 + 1) * cs;
    let interval = ep_interval(speed, binterval);
    w32(ep, interval << 16);
    let ty = if din { 7u32 } else { 3 };
    w32(ep + 4, ((maxpkt as u32) << 16) | (ty << 3) | (3 << 1));
    w64(ep + 8, h.intr.base | h.intr.c as u64);
    w32(ep + 16, 8);
    if !command(
        h,
        h.in_ctx,
        0,
        (TRB_CONFIG_EP << 10) | ((slot as u32) << 24),
    ) {
        return false;
    }
    h.kbd_dci = dci as u8;
    true
}

fn parse_hid(h: &Host) -> Option<(u8, u8, u16, u8, u8)> {
    // iface, ep_addr, maxpkt, binterval, cfg_value
    unsafe {
        let p = h.data as *const u8;
        let total = p.add(2).read() as u16 | ((p.add(3).read() as u16) << 8);
        let cfg_val = p.add(5).read();
        let mut off = 0u16;
        let mut iface = 0u8;
        let mut want = false;
        let mut found = None;
        while off + 2 <= total && (off as usize) < PAGE {
            let len = p.add(off as usize).read();
            if len < 2 {
                break;
            }
            let ty = p.add(off as usize + 1).read();
            if ty == 4 && len >= 9 {
                iface = p.add(off as usize + 2).read();
                let class = p.add(off as usize + 5).read();
                let sub = p.add(off as usize + 6).read();
                let proto = p.add(off as usize + 7).read();
                want = class == 3 && sub == 1 && proto == 1;
            } else if ty == 5 && len >= 7 && want {
                let addr = p.add(off as usize + 2).read();
                let attr = p.add(off as usize + 3).read();
                let maxpkt = p.add(off as usize + 4).read() as u16
                    | ((p.add(off as usize + 5).read() as u16) << 8);
                let interval = p.add(off as usize + 6).read();
                if attr & 3 == 3 && addr & 0x80 != 0 {
                    found = Some((iface, addr, maxpkt, interval, cfg_val));
                    break;
                }
            }
            off = off.saturating_add(len as u16);
        }
        found
    }
}

fn reset_port(h: &Host, port: u32) -> Option<u32> {
    let a = portsc(h.op, port);
    let v = r32(a);
    if v & CCS == 0 {
        return None;
    }
    if h.ppc && v & PP == 0 {
        w32(a, v | PP);
        let _ = wait_set(a, PP);
    }
    w32(a, r32(a) | PR | PORT_CHG);
    if !wait_set(a, PRC) {
        w32(a, r32(a) | WPR | PORT_CHG);
        if !wait_set(a, WRC) {
            return None;
        }
    }
    w32(a, r32(a) | PORT_CHG);
    if !wait_set(a, PED) {
        return None;
    }
    let speed = (r32(a) >> 10) & 0xF;
    if speed == 0 {
        None
    } else {
        Some(speed)
    }
}

fn take_legacy(bar: u64, xecp: u32) {
    if xecp == 0 {
        return;
    }
    let mut off = xecp as u64 * 4;
    let mut hops = 0u32;
    while off != 0 && hops < 32 {
        let cap = r32(bar + off);
        let id = cap & 0xFF;
        let next = (cap >> 8) & 0xFF;
        if id == 1 {
            w32(bar + off, cap | (1 << 24));
            let mut n = 0u32;
            while n < SPINS {
                if r32(bar + off) & (1 << 16) == 0 {
                    break;
                }
                n = n.saturating_add(1);
                core::hint::spin_loop();
            }
            w32(bar + off + 4, 0);
            return;
        }
        if next == 0 {
            break;
        }
        off = next as u64 * 4;
        hops = hops.saturating_add(1);
    }
}

fn try_kbd(h: &mut Host, port: u32, speed: u32) -> bool {
    let Some(slot) = enable_slot(h) else {
        return false;
    };
    if !address_device(h, slot, port, speed) {
        return false;
    }
    if !get_desc(h, slot, 1, 0, 18) {
        return false;
    }
    if !get_desc(h, slot, 2, 0, 9) {
        return false;
    }
    let total = unsafe {
        let p = h.data as *const u8;
        p.add(2).read() as u16 | ((p.add(3).read() as u16) << 8)
    };
    if total < 9 || total as usize > PAGE {
        return false;
    }
    if !get_desc(h, slot, 2, 0, total) {
        return false;
    }
    let Some((iface, ep, maxpkt, interval, cfg)) = parse_hid(h) else {
        return false;
    };
    if !set_config(h, slot, cfg) {
        return false;
    }
    let _ = hid_set(h, slot, iface, 0x0A, 0);
    let _ = hid_set(h, slot, iface, 0x0B, 0);
    if !config_intr(h, slot, port, speed, ep, maxpkt.max(8), interval) {
        return false;
    }
    h.kbd_slot = slot;
    h.kbd_prev = [0; 6];
    let _ = h.intr.enq(h.report, 8, (TRB_NORMAL << 10) | IOC | ISP);
    doorbell(h, slot as u32, h.kbd_dci as u32);
    true
}

fn bringup(bar: u64, len: u64) -> bool {
    if len == 0 || len > 64 * 1024 * 1024 {
        return false;
    }
    crate::mm::map_uc(bar, len);
    serial_print("kindling: xhci 0x");
    serial_hex(bar);
    serial_print("\n");
    let caplen = r8(bar) as u64;
    if caplen < 0x20 {
        return false;
    }
    let hcs1 = r32(bar + 4);
    let hcs2 = r32(bar + 8);
    let hcc1 = r32(bar + 0x10);
    let dboff = r32(bar + 0x14) as u64;
    let rtsoff = r32(bar + 0x18) as u64;
    if dboff == 0 || rtsoff == 0 {
        return false;
    }
    let max_slots = (hcs1 & 0xFF).min(16);
    let max_ports = (hcs1 >> 24) & 0xFF;
    if max_ports == 0 {
        return false;
    }
    let scratch = ((hcs2 >> 27) & 0x1F) | (((hcs2 >> 21) & 0x1F) << 5);
    let ctxsz = if hcc1 & 4 != 0 { 64u32 } else { 32 };
    let ppc = hcc1 & 8 != 0;
    let xecp = (hcc1 >> 16) & 0xFFFF;
    take_legacy(bar, xecp);
    let op = bar + caplen;
    let rt = bar + rtsoff;
    let db = bar + dboff;
    if r32(op) & RS != 0 {
        w32(op, r32(op) & !RS);
        if !wait_eq(op + 4, HCH, HCH) {
            return false;
        }
    }
    w32(op, HCRST);
    if !wait_eq(op, HCRST, 0) {
        return false;
    }
    if !wait_eq(op + 4, CNR, 0) {
        return false;
    }
    let dcbaa = page();
    let cmd_page = page();
    let evt_page = page();
    let erst = page();
    let in_ctx = page();
    let out_ctx = page();
    let ep0_page = page();
    let intr_page = page();
    let data = page();
    let report = page();
    if scratch > 0 {
        let arr = page();
        let mut i = 0u32;
        while i < scratch {
            w64(arr + i as u64 * 8, page());
            i = i.saturating_add(1);
        }
        w64(dcbaa, arr);
    }
    w32(op + 0x38, max_slots);
    w64(op + 0x30, dcbaa);
    let cmd = ring_new(cmd_page);
    w64(op + 0x18, cmd_page | 1);
    w32(op + 0x14, 0xFFFF);
    w32(erst, evt_page as u32);
    w32(erst + 4, (evt_page >> 32) as u32);
    w32(erst + 8, RING);
    w32(erst + 12, 0);
    w32(rt + 0x28, 1);
    w64(rt + 0x30, erst);
    w64(rt + 0x38, evt_page);
    w32(op, RS);
    if !wait_eq(op + 4, HCH, 0) {
        return false;
    }
    let mut h = Host {
        op,
        rt,
        db,
        ctxsz,
        max_ports,
        ppc,
        cmd,
        evt: Evt {
            base: evt_page,
            i: 0,
            c: 1,
        },
        dcbaa,
        in_ctx,
        out_ctx,
        ep0: ring_new(ep0_page),
        intr: ring_new(intr_page),
        data,
        report,
        cmd_seen: false,
        cmd_code: 0,
        cmd_slot: 0,
        xfer_seen: false,
        xfer_code: 0,
        kbd_slot: 0,
        kbd_dci: 0,
        kbd_prev: [0; 6],
    };
    let mut port = 1u32;
    while port <= h.max_ports {
        process_events(&mut h);
        if let Some(speed) = reset_port(&h, port) {
            if try_kbd(&mut h, port, speed) {
                *host_mut() = Some(h);
                unsafe {
                    core::ptr::addr_of_mut!(KBD).write(true);
                }
                return true;
            }
        }
        port = port.saturating_add(1);
    }
    false
}

/// After the well. Maps the BAR, takes the HC, looks for a boot keyboard.
pub fn init() {
    *host_mut() = None;
    unsafe {
        core::ptr::addr_of_mut!(KBD).write(false);
    }
    let mut found = [crate::pci::XhciBar { bar: 0, len: 0 }; 4];
    let n = crate::pci::iter_xhci(&mut found);
    let mut i = 0usize;
    while i < n {
        if found[i].bar != 0 && bringup(found[i].bar, found[i].len) {
            return;
        }
        i += 1;
    }
}

pub fn kbd_live() -> bool {
    unsafe { core::ptr::addr_of!(KBD).read() }
}

/// Drain the event ring. Safe to call when USB never came up.
pub fn poll() {
    if !kbd_live() {
        return;
    }
    if let Some(h) = host_mut().as_mut() {
        process_events(h);
    }
}
