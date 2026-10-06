//! xHCI + HID boot keyboard + MSC (BOT). Event ring is polled.

use core::cell::UnsafeCell;
use core::sync::atomic::{compiler_fence, fence, Ordering};

use crate::start::{serial_hex, serial_print};

const PAGE: usize = 4096;
const SPINS: u32 = 4_000_000;
const RING: u32 = 256;
const SLOTS: usize = 17;
const PORTS: usize = 32;

const TRB_NORMAL: u32 = 1;
const TRB_SETUP: u32 = 2;
const TRB_DATA: u32 = 3;
const TRB_STATUS: u32 = 4;
const TRB_LINK: u32 = 6;
const TRB_ENABLE_SLOT: u32 = 9;
const TRB_ADDRESS: u32 = 11;
const TRB_CONFIG_EP: u32 = 12;
const TRB_EVAL: u32 = 13;
const TRB_RESET_EP: u32 = 14;
const TRB_SET_DEQ: u32 = 16;
const TRB_XFER: u32 = 32;
const TRB_CMD: u32 = 33;
const TRB_PORT: u32 = 34;

const IOC: u32 = 1 << 5;
const IDT: u32 = 1 << 6;
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
const CAS: u32 = 1 << 24;

const T_SHORT: u64 = 20;
const T_CMD: u64 = 100;
const T_RST: u64 = 100;
const T_BIOS: u64 = 1000;

const FEAT_PORT_RESET: u16 = 4;
const FEAT_PORT_POWER: u16 = 8;
const FEAT_C_PORT_RESET: u16 = 20;

#[derive(Clone, Copy)]
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
    slot_ep0: [Ring; SLOTS],
    intr: Ring,
    data: u64,
    report: u64,
    port_st: [u8; PORTS],
    st_usb2: u32,
    st_usb3: u32,
    cmd_seen: bool,
    cmd_code: u32,
    cmd_slot: u32,
    xfer_seen: bool,
    xfer_code: u32,
    kbd_slot: u8,
    kbd_dci: u8,
    kbd_prev: [u8; 6],
    bulk_out: Ring,
    bulk_in: Ring,
    msc_slot: u8,
    msc_out_dci: u8,
    msc_in_dci: u8,
    msc_tag: u32,
    fat: bool,
}

struct HostCell(UnsafeCell<Option<Host>>);
unsafe impl Sync for HostCell {}

static HOST: HostCell = HostCell(UnsafeCell::new(None));
static mut KBD: bool = false;
static mut MSC: bool = false;
static mut FAT: bool = false;
static mut FOUND: bool = false;
/// How far bringup got: CCS, reset, address, desc, BOT, capacity.
static mut MISS: u8 = 0;
const M_CCS: u8 = 1;
const M_RST: u8 = 2;
const M_ADDR: u8 = 3;
const M_DEV: u8 = 4;
const M_BOT: u8 = 5;

fn host_mut() -> &'static mut Option<Host> {
    unsafe { &mut *HOST.0.get() }
}

fn r8(a: u64) -> u8 {
    unsafe { (a as *const u8).read_volatile() }
}
fn r32(a: u64) -> u32 {
    unsafe { (a as *const u32).read_volatile() }
}
fn w8(a: u64, v: u8) {
    unsafe { (a as *mut u8).write_volatile(v) }
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

fn wait_loop(timeout_ms: u64, mut ok: impl FnMut() -> bool) -> bool {
    if crate::timer::clock_live() {
        let t0 = crate::timer::ms();
        while crate::timer::ms().saturating_sub(t0) < timeout_ms {
            if ok() {
                return true;
            }
            core::hint::spin_loop();
        }
        return false;
    }
    let mut n = 0u32;
    while n < SPINS {
        if ok() {
            return true;
        }
        n = n.saturating_add(1);
        core::hint::spin_loop();
    }
    false
}

fn wait_eq(addr: u64, mask: u32, want: u32, timeout_ms: u64) -> bool {
    wait_loop(timeout_ms, || r32(addr) & mask == want)
}

fn wait_set(addr: u64, mask: u32, timeout_ms: u64) -> bool {
    wait_loop(timeout_ms, || r32(addr) & mask != 0)
}

fn pause() {
    let mut n = 0u32;
    while n < 200_000 {
        n = n.saturating_add(1);
        core::hint::spin_loop();
    }
}

fn bump(step: u8) {
    unsafe {
        let p = core::ptr::addr_of_mut!(MISS);
        if p.read() < step {
            p.write(step);
        }
    }
}

fn recover() {
    if crate::timer::clock_live() {
        crate::timer::sleep_ms(10);
    } else {
        pause();
    }
}

fn settle(h: &mut Host, ms: u64) {
    if crate::timer::clock_live() {
        let t0 = crate::timer::ms();
        while crate::timer::ms().saturating_sub(t0) < ms {
            process_events(h);
            core::hint::spin_loop();
        }
        return;
    }
    let mut n = 0u32;
    while n < 80 {
        process_events(h);
        pause();
        n = n.saturating_add(1);
    }
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
        self.write(param, status, ctrl, self.c)
    }

    /// Invert cycle so the HC cannot prefetch a partial control transfer.
    fn enq_held(&mut self, param: u64, status: u32, ctrl: u32) -> u64 {
        self.write(param, status, ctrl, self.c ^ 1)
    }

    fn write(&mut self, param: u64, status: u32, ctrl: u32, cycle: u32) -> u64 {
        let addr = self.base + self.i as u64 * 16;
        wr_trb(addr, param, status, ctrl | (cycle & 1));
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

fn give_trb(addr: u64, cycle: u32) {
    let ctrl = r32(addr + 12);
    w32(addr + 12, (ctrl & !1) | (cycle & 1));
}

fn portsc(op: u64, port: u32) -> u64 {
    op + 0x400 + (port as u64 - 1) * 0x10
}

/// Linux `xhci_port_state_to_neutral`. PED (bit 1) is RW1CS: writing 1
/// *disables* the port. It is not RO — do not put it in the write.
fn port_neutral(v: u32) -> u32 {
    let ro = CCS | (1 << 3) | (0xF << 10) | (1 << 30);
    let rws = (0xF << 5) | PP | (0x3 << 14) | (0x7 << 25);
    (v & ro) | (v & rws)
}

fn port_set(a: u64, bits: u32) {
    w32(a, port_neutral(r32(a)) | bits);
}

fn port_ack(a: u64) {
    w32(a, port_neutral(r32(a)) | PORT_CHG);
}

fn doorbell(h: &Host, slot: u32, target: u32) {
    fence(Ordering::SeqCst);
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
                let slot = (ctrl >> 24) as u8;
                let epid = ((ctrl >> 16) & 0x1F) as u8;
                if h.kbd_slot != 0 && slot == h.kbd_slot && epid == h.kbd_dci {
                    hid_report(h);
                } else {
                    h.xfer_seen = true;
                    h.xfer_code = status >> 24;
                }
            }
            TRB_PORT => {
                let port = ((param >> 24) & 0xFF) as u32;
                if (1..256).contains(&port) {
                    port_ack(portsc(h.op, port));
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
    wait_loop(T_CMD, || {
        process_events(h);
        h.cmd_seen
    }) && h.cmd_code == CC_OK
}

fn slot_type_for(h: &Host, speed: u32, root_port: u32) -> u32 {
    if (1..PORTS as u32).contains(&root_port) {
        let t = h.port_st[root_port as usize] as u32;
        if t != 0 {
            return t;
        }
    }
    if speed >= 4 {
        h.st_usb3
    } else {
        h.st_usb2
    }
}

fn enable_slot(h: &mut Host, ty: u32) -> Option<u8> {
    let try_ty = |h: &mut Host, ty: u32| -> Option<u8> {
        if !command(h, 0, 0, (TRB_ENABLE_SLOT << 10) | (ty << 16)) {
            return None;
        }
        let s = h.cmd_slot as u8;
        if s == 0 || (s as usize) >= SLOTS {
            None
        } else {
            Some(s)
        }
    };
    if let Some(s) = try_ty(h, ty) {
        return Some(s);
    }
    if ty != 0 {
        try_ty(h, 0)
    } else {
        None
    }
}

fn bind_slot(h: &mut Host, slot: u8) {
    let out = page();
    w64(h.dcbaa + slot as u64 * 8, out);
    let ep0 = page();
    h.slot_ep0[slot as usize] = ring_new(ep0);
}

fn fill_slot_ctx(h: &Host, speed: u32, root_port: u32, route: u32, tt_slot: u32, tt_port: u32, entries: u32) {
    let cs = h.ctxsz as u64;
    let slot_ctx = h.in_ctx + cs;
    w32(slot_ctx, (entries << 27) | (speed << 20) | (route & 0xF_FFFF));
    w32(slot_ctx + 4, root_port << 16);
    w32(slot_ctx + 8, (tt_slot & 0xFF) | ((tt_port & 0xFF) << 8));
}

fn ep0_mps(speed: u32) -> u32 {
    if speed >= 4 {
        512
    } else if speed == 3 {
        64
    } else {
        8
    }
}

fn fill_ep0(h: &Host, mps: u32, deq: u64, dcs: u32) {
    let ep0 = h.in_ctx + 2 * h.ctxsz as u64;
    w32(ep0 + 4, (mps << 16) | (4 << 3) | (3 << 1));
    w64(ep0 + 8, deq | (dcs as u64 & 1));
    w32(ep0 + 16, 8);
}

fn address_device(
    h: &mut Host,
    slot: u8,
    root_port: u32,
    speed: u32,
    route: u32,
    tt_slot: u32,
    tt_port: u32,
) -> bool {
    bind_slot(h, slot);
    unsafe {
        core::ptr::write_bytes(h.in_ctx as *mut u8, 0, PAGE);
    }
    w32(h.in_ctx + 4, 0b11);
    fill_slot_ctx(h, speed, root_port, route, tt_slot, tt_port, 1);
    let ring = h.slot_ep0[slot as usize];
    fill_ep0(h, ep0_mps(speed), ring.base, ring.c);
    command(
        h,
        h.in_ctx,
        0,
        (TRB_ADDRESS << 10) | ((slot as u32) << 24),
    )
}

fn evaluate_ep0(
    h: &mut Host,
    slot: u8,
    root_port: u32,
    speed: u32,
    route: u32,
    tt_slot: u32,
    tt_port: u32,
    mps: u32,
) -> bool {
    unsafe {
        core::ptr::write_bytes(h.in_ctx as *mut u8, 0, PAGE);
    }
    w32(h.in_ctx + 4, 0b11);
    fill_slot_ctx(h, speed, root_port, route, tt_slot, tt_port, 1);
    let ring = h.slot_ep0[slot as usize];
    let deq = ring.base + ring.i as u64 * 16;
    fill_ep0(h, mps, deq, ring.c);
    command(
        h,
        h.in_ctx,
        0,
        (TRB_EVAL << 10) | ((slot as u32) << 24),
    )
}

fn heal_ep0(h: &mut Host, slot: u8) {
    let _ = command(
        h,
        0,
        0,
        (TRB_RESET_EP << 10) | (1 << 16) | ((slot as u32) << 24),
    );
    let ring = h.slot_ep0[slot as usize];
    let deq = ring.base + ring.i as u64 * 16;
    let _ = command(
        h,
        deq | (ring.c as u64 & 1),
        0,
        (TRB_SET_DEQ << 10) | (1 << 16) | ((slot as u32) << 24),
    );
    recover();
}

/// Setup, Data, and Status are separate TDs (xHCI 4.11.2.2). TRT IN=3, OUT=2.
/// The first TRB stays software-owned until the rest are written — Intel
/// prefetches EP0 as soon as Address Device leaves the endpoint Running.
fn control(h: &mut Host, slot: u8, setup: [u8; 8], buf: u64, len: u16, din: bool) -> bool {
    let pkt = u64::from_le_bytes(setup);
    let trt: u32 = if len == 0 {
        0
    } else if din {
        3
    } else {
        2
    };
    h.xfer_seen = false;
    let c0 = h.slot_ep0[slot as usize].c;
    let setup_addr = h.slot_ep0[slot as usize].enq_held(
        pkt,
        8,
        (TRB_SETUP << 10) | IDT | (trt << 16),
    );
    if len != 0 {
        let dir = if din { 1u32 << 16 } else { 0 };
        let _ = h.slot_ep0[slot as usize].enq(buf, len as u32, (TRB_DATA << 10) | dir);
    }
    let sdir = if din && len != 0 { 0 } else { 1u32 << 16 };
    let _ = h.slot_ep0[slot as usize].enq(0, 0, (TRB_STATUS << 10) | IOC | sdir);
    give_trb(setup_addr, c0);
    doorbell(h, slot as u32, 1);
    let ok = wait_xfer(h);
    if !ok {
        heal_ep0(h, slot);
    }
    ok
}

fn wait_xfer(h: &mut Host) -> bool {
    wait_loop(T_CMD, || {
        process_events(h);
        h.xfer_seen
    }) && (h.xfer_code == CC_OK || h.xfer_code == CC_SHORT)
}

fn get_desc(h: &mut Host, slot: u8, ty: u8, idx: u8, len: u16) -> bool {
    let mut s = [0u8; 8];
    s[0] = 0x80;
    s[1] = 6;
    s[2] = idx;
    s[3] = ty;
    s[6] = (len & 0xFF) as u8;
    s[7] = (len >> 8) as u8;
    let mut n = 0u32;
    while n < 2 {
        unsafe {
            core::ptr::write_bytes(h.data as *mut u8, 0, PAGE);
        }
        if control(h, slot, s, h.data, len, true) {
            return true;
        }
        n = n.saturating_add(1);
    }
    false
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

fn hub_feat(h: &mut Host, slot: u8, set: bool, feat: u16, port: u8) -> bool {
    let mut s = [0u8; 8];
    s[0] = 0x23;
    s[1] = if set { 3 } else { 1 };
    s[2] = (feat & 0xFF) as u8;
    s[3] = (feat >> 8) as u8;
    s[4] = port;
    control(h, slot, s, 0, 0, false)
}

fn hub_port_status(h: &mut Host, slot: u8, port: u8) -> Option<u32> {
    let mut s = [0u8; 8];
    s[0] = 0xA3;
    s[4] = port;
    s[6] = 4;
    unsafe {
        core::ptr::write_bytes(h.data as *mut u8, 0, 8);
    }
    if !control(h, slot, s, h.data, 4, true) {
        return None;
    }
    Some(unsafe { (h.data as *const u32).read_volatile() })
}

fn hub_desc(h: &mut Host, slot: u8) -> Option<u8> {
    let mut s = [0u8; 8];
    s[0] = 0xA0;
    s[1] = 6;
    s[3] = 0x29;
    s[6] = 12;
    unsafe {
        core::ptr::write_bytes(h.data as *mut u8, 0, 16);
    }
    if !control(h, slot, s, h.data, 12, true) {
        return None;
    }
    Some(unsafe { (h.data as *const u8).add(2).read() })
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
    root_port: u32,
    speed: u32,
    route: u32,
    tt_slot: u32,
    tt_port: u32,
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
    fill_slot_ctx(h, speed, root_port, route, tt_slot, tt_port, dci);
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

/// iface, ep_addr, maxpkt, binterval, cfg_value
fn parse_hid(h: &Host) -> Option<(u8, u8, u16, u8, u8)> {
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
                let proto = p.add(off as usize + 7).read();
                want = class == 3 && proto == 1;
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

fn cfg_value(h: &Host) -> u8 {
    unsafe { (h.data as *const u8).add(5).read() }
}

fn is_hub_iface(h: &Host) -> bool {
    unsafe {
        let p = h.data as *const u8;
        let total = p.add(2).read() as u16 | ((p.add(3).read() as u16) << 8);
        let mut off = 0u16;
        while off + 2 <= total && (off as usize) < PAGE {
            let len = p.add(off as usize).read();
            if len < 2 {
                break;
            }
            let ty = p.add(off as usize + 1).read();
            if ty == 4 && len >= 9 && p.add(off as usize + 5).read() == 9 {
                return true;
            }
            off = off.saturating_add(len as u16);
        }
        false
    }
}

fn get_config(h: &mut Host, slot: u8) -> bool {
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
    get_desc(h, slot, 2, 0, total)
}

fn try_hid(
    h: &mut Host,
    slot: u8,
    root_port: u32,
    speed: u32,
    route: u32,
    tt_slot: u32,
    tt_port: u32,
) -> bool {
    if h.kbd_slot != 0 {
        return false;
    }
    if !get_config(h, slot) {
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
    if !config_intr(
        h,
        slot,
        root_port,
        speed,
        route,
        tt_slot,
        tt_port,
        ep,
        maxpkt.max(8),
        interval,
    ) {
        return false;
    }
    h.kbd_slot = slot;
    h.kbd_prev = [0; 6];
    let _ = h.intr.enq(h.report, 8, (TRB_NORMAL << 10) | IOC | ISP);
    doorbell(h, slot as u32, h.kbd_dci as u32);
    true
}

const CBW_SIG: u32 = 0x4342_5355;
const CSW_SIG: u32 = 0x5342_5355;
const MSC_CSW: u64 = 64;
const MSC_DATA: u64 = 128;

/// iface, out_ep, in_ep, out_max, in_max, cfg_value
fn parse_msc(h: &Host) -> Option<(u8, u8, u8, u16, u16, u8)> {
    unsafe {
        let p = h.data as *const u8;
        let total = p.add(2).read() as u16 | ((p.add(3).read() as u16) << 8);
        let cfg_val = p.add(5).read();
        let mut off = 0u16;
        let mut iface = 0u8;
        let mut want = false;
        let mut out_ep = 0u8;
        let mut in_ep = 0u8;
        let mut out_max = 0u16;
        let mut in_max = 0u16;
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
                want = class == 8 && sub == 6 && proto == 0x50;
                out_ep = 0;
                in_ep = 0;
            } else if ty == 5 && len >= 7 && want {
                let addr = p.add(off as usize + 2).read();
                let attr = p.add(off as usize + 3).read();
                let maxpkt = p.add(off as usize + 4).read() as u16
                    | ((p.add(off as usize + 5).read() as u16) << 8);
                if attr & 3 == 2 {
                    if addr & 0x80 == 0 {
                        out_ep = addr;
                        out_max = maxpkt;
                    } else {
                        in_ep = addr;
                        in_max = maxpkt;
                    }
                    if out_ep != 0 && in_ep != 0 {
                        return Some((iface, out_ep, in_ep, out_max, in_max, cfg_val));
                    }
                }
            }
            off = off.saturating_add(len as u16);
        }
        None
    }
}

fn config_bulk(
    h: &mut Host,
    slot: u8,
    root_port: u32,
    speed: u32,
    route: u32,
    tt_slot: u32,
    tt_port: u32,
    out_ep: u8,
    in_ep: u8,
    out_max: u16,
    in_max: u16,
) -> bool {
    let out_num = (out_ep & 0x0F) as u32;
    let in_num = (in_ep & 0x0F) as u32;
    if out_num == 0 || in_num == 0 {
        return false;
    }
    let out_dci = out_num * 2;
    let in_dci = in_num * 2 + 1;
    let def_pkt: u16 = if speed >= 3 { 512 } else { 64 };
    let out_max = if out_max == 0 { def_pkt } else { out_max };
    let in_max = if in_max == 0 { def_pkt } else { in_max };
    unsafe {
        core::ptr::write_bytes(h.in_ctx as *mut u8, 0, PAGE);
        core::ptr::write_bytes(h.bulk_out.base as *mut u8, 0, PAGE);
        core::ptr::write_bytes(h.bulk_in.base as *mut u8, 0, PAGE);
    }
    h.bulk_out = ring_new(h.bulk_out.base);
    h.bulk_in = ring_new(h.bulk_in.base);
    let cs = h.ctxsz as u64;
    let entries = out_dci.max(in_dci);
    w32(h.in_ctx + 4, (1 << out_dci) | (1 << in_dci) | 1);
    fill_slot_ctx(h, speed, root_port, route, tt_slot, tt_port, entries);
    let ep_out = h.in_ctx + (out_dci as u64 + 1) * cs;
    w32(ep_out + 4, ((out_max as u32) << 16) | (2 << 3) | (3 << 1));
    w64(ep_out + 8, h.bulk_out.base | h.bulk_out.c as u64);
    w32(ep_out + 16, out_max as u32);
    let ep_in = h.in_ctx + (in_dci as u64 + 1) * cs;
    w32(ep_in + 4, ((in_max as u32) << 16) | (6 << 3) | (3 << 1));
    w64(ep_in + 8, h.bulk_in.base | h.bulk_in.c as u64);
    w32(ep_in + 16, in_max as u32);
    if !command(
        h,
        h.in_ctx,
        0,
        (TRB_CONFIG_EP << 10) | ((slot as u32) << 24),
    ) {
        return false;
    }
    h.msc_out_dci = out_dci as u8;
    h.msc_in_dci = in_dci as u8;
    true
}

fn msc_max_lun(h: &mut Host, slot: u8, iface: u8) {
    let mut s = [0u8; 8];
    s[0] = 0xA1;
    s[1] = 0xFE;
    s[4] = iface;
    s[6] = 1;
    let _ = control(h, slot, s, h.data, 1, true);
}

fn bulk_out(h: &mut Host, buf: u64, len: u32) -> bool {
    let slot = h.msc_slot;
    let dci = h.msc_out_dci;
    let _ = h.bulk_out.enq(buf, len, (TRB_NORMAL << 10) | IOC);
    h.xfer_seen = false;
    doorbell(h, slot as u32, dci as u32);
    wait_xfer(h)
}

fn bulk_in(h: &mut Host, buf: u64, len: u32) -> bool {
    let slot = h.msc_slot;
    let dci = h.msc_in_dci;
    let _ = h.bulk_in.enq(buf, len, (TRB_NORMAL << 10) | IOC | ISP);
    h.xfer_seen = false;
    doorbell(h, slot as u32, dci as u32);
    wait_xfer(h)
}

fn write_cbw(addr: u64, tag: u32, data_len: u32, din: bool, cdb: &[u8]) {
    unsafe {
        core::ptr::write_bytes(addr as *mut u8, 0, 31);
    }
    w32(addr, CBW_SIG);
    w32(addr + 4, tag);
    w32(addr + 8, data_len);
    w8(addr + 12, if din { 0x80 } else { 0 });
    w8(addr + 13, 0);
    let n = cdb.len().min(16);
    w8(addr + 14, n as u8);
    let mut i = 0usize;
    while i < n {
        w8(addr + 15 + i as u64, cdb[i]);
        i += 1;
    }
}

fn bot(h: &mut Host, cdb: &[u8], buf: u64, data_len: u32, din: bool) -> bool {
    let tag = h.msc_tag;
    h.msc_tag = h.msc_tag.wrapping_add(1);
    if h.msc_tag == 0 {
        h.msc_tag = 1;
    }
    write_cbw(h.data, tag, data_len, din && data_len != 0, cdb);
    if !bulk_out(h, h.data, 31) {
        return false;
    }
    if data_len != 0 {
        let ok = if din {
            bulk_in(h, buf, data_len)
        } else {
            bulk_out(h, buf, data_len)
        };
        if !ok {
            return false;
        }
    }
    unsafe {
        core::ptr::write_bytes((h.data + MSC_CSW) as *mut u8, 0, 16);
    }
    if !bulk_in(h, h.data + MSC_CSW, 13) {
        return false;
    }
    let sig = r32(h.data + MSC_CSW);
    let stag = r32(h.data + MSC_CSW + 4);
    let status = r8(h.data + MSC_CSW + 12);
    sig == CSW_SIG && stag == tag && status == 0
}

fn looks_fat(p: u64) -> bool {
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

fn msc_read10(h: &mut Host, lba: u32, buf: u64, blocks: u16, blk: u32) -> bool {
    if blocks == 0 || blk == 0 {
        return false;
    }
    let bytes = blk.saturating_mul(blocks as u32);
    if bytes == 0 || bytes as usize > PAGE {
        return false;
    }
    let cdb = [
        0x28,
        0,
        (lba >> 24) as u8,
        (lba >> 16) as u8,
        (lba >> 8) as u8,
        lba as u8,
        0,
        (blocks >> 8) as u8,
        blocks as u8,
        0,
    ];
    bot(h, &cdb, buf, bytes, true)
}

fn msc_ready(h: &mut Host) -> bool {
    let tur = [0u8; 6];
    let mut n = 0u32;
    while n < 16 {
        if bot(h, &tur, 0, 0, false) {
            return true;
        }
        let sense = [0x03, 0, 0, 0, 18, 0];
        let _ = bot(h, &sense, h.data + MSC_DATA, 18, true);
        pause();
        n = n.saturating_add(1);
    }
    false
}

fn try_msc(
    h: &mut Host,
    slot: u8,
    root_port: u32,
    speed: u32,
    route: u32,
    tt_slot: u32,
    tt_port: u32,
) -> bool {
    if h.msc_slot != 0 {
        return false;
    }
    if !get_config(h, slot) {
        return false;
    }
    let Some((iface, out_ep, in_ep, out_max, in_max, cfg)) = parse_msc(h) else {
        return false;
    };
    if !set_config(h, slot, cfg) {
        return false;
    }
    let _ = msc_max_lun(h, slot, iface);
    if !config_bulk(
        h,
        slot,
        root_port,
        speed,
        route,
        tt_slot,
        tt_port,
        out_ep,
        in_ep,
        out_max,
        in_max,
    ) {
        return false;
    }
    bump(M_BOT);
    h.msc_slot = slot;
    h.msc_tag = 1;
    recover();
    let _ = msc_ready(h);
    let cap = [0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    if !bot(h, &cap, h.data + MSC_DATA, 8, true) {
        h.msc_slot = 0;
        return false;
    }
    let blk = (r8(h.data + MSC_DATA + 4) as u32) << 24
        | (r8(h.data + MSC_DATA + 5) as u32) << 16
        | (r8(h.data + MSC_DATA + 6) as u32) << 8
        | r8(h.data + MSC_DATA + 7) as u32;
    if blk == 512
        && msc_read10(h, 0, h.data + MSC_DATA, 1, 512)
        && looks_fat(h.data + MSC_DATA)
    {
        h.fat = true;
    }
    true
}

fn hub_child_speed(status: u32) -> u32 {
    // wPortStatus: bit 10 LS, bit 11 HS; else FS. SS hubs use different bits.
    if status & (1 << 10) != 0 {
        2
    } else if status & (1 << 11) != 0 {
        3
    } else {
        1
    }
}

fn evaluate_hub(h: &mut Host, slot: u8, root_port: u32, speed: u32, nports: u32) -> bool {
    unsafe {
        core::ptr::write_bytes(h.in_ctx as *mut u8, 0, PAGE);
    }
    let cs = h.ctxsz as u64;
    w32(h.in_ctx + 4, 1);
    fill_slot_ctx(h, speed, root_port, 0, 0, 0, 1);
    let slot_ctx = h.in_ctx + cs;
    w32(slot_ctx, r32(slot_ctx) | (1 << 26));
    w32(slot_ctx + 4, r32(slot_ctx + 4) | (nports << 24));
    command(
        h,
        h.in_ctx,
        0,
        (TRB_EVAL << 10) | ((slot as u32) << 24),
    )
}

fn hub_wait(h: &mut Host, slot: u8, port: u8, mask: u32, want: u32) -> Option<u32> {
    let mut n = 0u32;
    while n < 32 {
        if let Some(st) = hub_port_status(h, slot, port) {
            if st & mask == want {
                return Some(st);
            }
        }
        pause();
        n = n.saturating_add(1);
    }
    hub_port_status(h, slot, port)
}

fn try_hub(
    h: &mut Host,
    hub_slot: u8,
    root_port: u32,
    hub_speed: u32,
) -> bool {
    if !get_config(h, hub_slot) {
        return false;
    }
    let cfg = cfg_value(h);
    if !set_config(h, hub_slot, cfg) {
        return false;
    }
    let Some(nports) = hub_desc(h, hub_slot) else {
        return false;
    };
    let nports = nports.min(15);
    let _ = evaluate_hub(h, hub_slot, root_port, hub_speed, nports as u32);
    let mut hp = 1u8;
    while hp <= nports {
        let _ = hub_feat(h, hub_slot, true, FEAT_PORT_POWER, hp);
        let Some(st) = hub_wait(h, hub_slot, hp, 1, 1) else {
            hp = hp.saturating_add(1);
            continue;
        };
        if st & 1 == 0 {
            hp = hp.saturating_add(1);
            continue;
        }
        let _ = hub_feat(h, hub_slot, true, FEAT_PORT_RESET, hp);
        let Some(st) = hub_wait(h, hub_slot, hp, 2, 2) else {
            hp = hp.saturating_add(1);
            continue;
        };
        let _ = hub_feat(h, hub_slot, false, FEAT_C_PORT_RESET, hp);
        if st & 2 == 0 {
            hp = hp.saturating_add(1);
            continue;
        }
        let speed = hub_child_speed(st);
        let ty = slot_type_for(h, speed, root_port);
        let Some(child) = enable_slot(h, ty) else {
            hp = hp.saturating_add(1);
            continue;
        };
        let tt_slot = if hub_speed >= 3 && speed <= 2 {
            hub_slot as u32
        } else {
            0
        };
        let tt_port = if tt_slot != 0 { hp as u32 } else { 0 };
        if address_device(h, child, root_port, speed, hp as u32, tt_slot, tt_port) {
            bump(M_ADDR);
            recover();
            let _ = try_msc(h, child, root_port, speed, hp as u32, tt_slot, tt_port);
            let _ = try_hid(h, child, root_port, speed, hp as u32, tt_slot, tt_port);
        }
        if h.kbd_slot != 0 && h.msc_slot != 0 {
            break;
        }
        hp = hp.saturating_add(1);
    }
    h.kbd_slot != 0 || h.msc_slot != 0
}

fn fetch_dev(
    h: &mut Host,
    slot: u8,
    root_port: u32,
    speed: u32,
    route: u32,
    tt_slot: u32,
    tt_port: u32,
) -> bool {
    if speed < 3 {
        if get_desc(h, slot, 1, 0, 8) {
            let bmax = r8(h.data + 7) as u32;
            let mps = if bmax == 0 { 8 } else { bmax };
            let _ = evaluate_ep0(h, slot, root_port, speed, route, tt_slot, tt_port, mps);
        }
    }
    if get_desc(h, slot, 1, 0, 18) {
        return true;
    }
    let _ = evaluate_ep0(h, slot, root_port, speed, route, tt_slot, tt_port, 64);
    if get_desc(h, slot, 1, 0, 18) {
        return true;
    }
    let _ = evaluate_ep0(h, slot, root_port, speed, route, tt_slot, tt_port, 8);
    get_desc(h, slot, 1, 0, 18)
}

fn try_tree(
    h: &mut Host,
    slot: u8,
    root_port: u32,
    speed: u32,
    route: u32,
    tt_slot: u32,
    tt_port: u32,
) -> bool {
    if !fetch_dev(h, slot, root_port, speed, route, tt_slot, tt_port) {
        return false;
    }
    bump(M_DEV);
    let class = unsafe { (h.data as *const u8).add(4).read() };
    if class == 9 {
        return try_hub(h, slot, root_port, speed);
    }
    let mut ok = false;
    if try_msc(h, slot, root_port, speed, route, tt_slot, tt_port) {
        ok = true;
    }
    if try_hid(h, slot, root_port, speed, route, tt_slot, tt_port) {
        ok = true;
    }
    if h.kbd_slot == 0 || h.msc_slot == 0 {
        if get_config(h, slot) && is_hub_iface(h) {
            ok |= try_hub(h, slot, root_port, speed);
        }
    }
    ok
}

fn power_ports(h: &mut Host) {
    if !h.ppc {
        return;
    }
    let mut port = 1u32;
    while port <= h.max_ports {
        let a = portsc(h.op, port);
        port_set(a, PP);
        port = port.saturating_add(1);
    }
    let mut port = 1u32;
    while port <= h.max_ports {
        let _ = wait_set(portsc(h.op, port), PP, T_SHORT);
        port = port.saturating_add(1);
    }
    process_events(h);
}

fn reset_port(h: &Host, port: u32) -> Option<u32> {
    let a = portsc(h.op, port);
    let v = r32(a);
    if v & CCS == 0 {
        return None;
    }
    if h.ppc && v & PP == 0 {
        return None;
    }
    bump(M_CCS);
    port_ack(a);
    let v = r32(a);
    let ss_port = (port as usize) < PORTS
        && h.st_usb3 != 0
        && h.port_st[port as usize] == h.st_usb3 as u8;
    let speed0 = (v >> 10) & 0xF;
    let warm = ss_port || speed0 >= 4 || v & CAS != 0;
    if warm {
        port_set(a, WPR);
        if !wait_set(a, WRC, T_RST) && !wait_set(a, PED, T_RST) {
            return None;
        }
    } else {
        port_set(a, PR);
        if r32(a) & PR == 0 && r32(a) & PRC == 0 {
            w32(a, port_neutral(r32(a)) | PR);
        }
        if !wait_eq(a, PR, 0, T_RST) {
            return None;
        }
    }
    port_ack(a);
    if !wait_set(a, PED, T_RST) {
        return None;
    }
    let mut speed = (r32(a) >> 10) & 0xF;
    if speed == 0 {
        recover();
        speed = (r32(a) >> 10) & 0xF;
    }
    if speed == 0 {
        None
    } else {
        bump(M_RST);
        Some(speed)
    }
}

fn scan_root(h: &mut Host, tried: &mut [bool; PORTS]) {
    let mut port = 1u32;
    while port <= h.max_ports && (port as usize) < PORTS {
        process_events(h);
        let v = r32(portsc(h.op, port));
        if v & CCS != 0 {
            bump(M_CCS);
        }
        if tried[port as usize] || v & CCS == 0 {
            port = port.saturating_add(1);
            continue;
        }
        if let Some(speed) = reset_port(h, port) {
            tried[port as usize] = true;
            recover();
            let ty = slot_type_for(h, speed, port);
            if let Some(slot) = enable_slot(h, ty) {
                if address_device(h, slot, port, speed, 0, 0, 0) {
                    bump(M_ADDR);
                    recover();
                    let _ = try_tree(h, slot, port, speed, 0, 0, 0);
                }
            }
        }
        if h.kbd_slot != 0 && h.msc_slot != 0 {
            return;
        }
        port = port.saturating_add(1);
    }
}

fn parse_caps(bar: u64, xecp: u32, port_st: &mut [u8; PORTS], st2: &mut u32, st3: &mut u32) {
    if xecp == 0 {
        return;
    }
    // Next is a DWORD offset from this capability, not from the BAR.
    let mut off = xecp as u64;
    let mut hops = 0u32;
    while off != 0 && hops < 32 {
        let cap = r32(bar + off * 4);
        let id = cap & 0xFF;
        let next = (cap >> 8) & 0xFF;
        let major = (cap >> 24) & 0xFF;
        if id == 1 {
            w32(bar + off * 4, cap | (1 << 24));
            let _ = wait_eq(bar + off * 4, 1 << 16, 0, T_BIOS);
            w32(bar + off * 4 + 4, 0);
        } else if id == 2 {
            let d2 = r32(bar + off * 4 + 8);
            let poff = d2 & 0xFF;
            let pcnt = (d2 >> 8) & 0xFF;
            let sty = ((d2 >> 16) & 0xF) as u8;
            if major == 2 {
                *st2 = sty as u32;
            }
            if major == 3 {
                *st3 = sty as u32;
            }
            let mut p = poff;
            while p < poff + pcnt && (p as usize) < PORTS {
                if p != 0 {
                    port_st[p as usize] = sty;
                }
                p = p.saturating_add(1);
            }
        }
        if next == 0 {
            break;
        }
        off = off.saturating_add(next as u64);
        hops = hops.saturating_add(1);
    }
}

fn empty_rings() -> [Ring; SLOTS] {
    [Ring {
        base: 0,
        i: 0,
        c: 1,
    }; SLOTS]
}

fn bringup(bar: u64, len: u64) -> bool {
    if len == 0 || len > 64 * 1024 * 1024 {
        return false;
    }
    crate::mm::map_uc(bar, len);
    serial_print("kindling: xhci 0x");
    serial_hex(bar);
    serial_print("\n");
    if r32(bar) == 0xFFFF_FFFF {
        return false;
    }
    let caplen = r8(bar) as u64;
    if caplen < 0x20 || caplen > 0x80 {
        return false;
    }
    let hcs1 = r32(bar + 4);
    let hcs2 = r32(bar + 8);
    let hcc1 = r32(bar + 0x10);
    let dboff = r32(bar + 0x14) as u64;
    let rtsoff = r32(bar + 0x18) as u64;
    if dboff == 0 || rtsoff == 0 || dboff > len || rtsoff > len {
        return false;
    }
    let max_slots = (hcs1 & 0xFF).min(16);
    let max_ports = ((hcs1 >> 24) & 0xFF).min((PORTS as u32) - 1);
    if max_ports == 0 || hcs1 == 0xFFFF_FFFF {
        return false;
    }
    let scratch = ((hcs2 >> 27) & 0x1F) | (((hcs2 >> 21) & 0x1F) << 5);
    let ctxsz = if hcc1 & 4 != 0 { 64u32 } else { 32 };
    let ppc = hcc1 & 8 != 0;
    let xecp = (hcc1 >> 16) & 0xFFFF;
    let mut port_st = [0u8; PORTS];
    let mut st_usb2 = 0u32;
    let mut st_usb3 = 0u32;
    parse_caps(bar, xecp, &mut port_st, &mut st_usb2, &mut st_usb3);
    let op = bar + caplen;
    let rt = bar + rtsoff;
    let db = bar + dboff;
    if r32(op) & RS != 0 {
        w32(op, r32(op) & !RS);
        if !wait_eq(op + 4, HCH, HCH, T_RST) {
            return false;
        }
    }
    w32(op, HCRST);
    if !wait_eq(op, HCRST, 0, T_RST) {
        return false;
    }
    if !wait_eq(op + 4, CNR, 0, T_RST) {
        return false;
    }
    parse_caps(bar, xecp, &mut port_st, &mut st_usb2, &mut st_usb3);
    let dcbaa = page();
    let cmd_page = page();
    let evt_page = page();
    let erst = page();
    let in_ctx = page();
    let intr_page = page();
    let bulk_out_page = page();
    let bulk_in_page = page();
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
    w32(rt + 0x20, 2);
    w32(rt + 0x28, 1);
    w64(rt + 0x30, erst);
    w64(rt + 0x38, evt_page);
    w32(op, RS);
    if !wait_eq(op + 4, HCH, 0, T_RST) {
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
        slot_ep0: empty_rings(),
        intr: ring_new(intr_page),
        data,
        report,
        port_st,
        st_usb2,
        st_usb3,
        cmd_seen: false,
        cmd_code: 0,
        cmd_slot: 0,
        xfer_seen: false,
        xfer_code: 0,
        kbd_slot: 0,
        kbd_dci: 0,
        kbd_prev: [0; 6],
        bulk_out: ring_new(bulk_out_page),
        bulk_in: ring_new(bulk_in_page),
        msc_slot: 0,
        msc_out_dci: 0,
        msc_in_dci: 0,
        msc_tag: 1,
        fat: false,
    };
    power_ports(&mut h);
    settle(&mut h, 100);
    let mut tried = [false; PORTS];
    scan_root(&mut h, &mut tried);
    if h.msc_slot == 0 {
        settle(&mut h, 100);
        scan_root(&mut h, &mut tried);
    }
    if h.kbd_slot == 0 && h.msc_slot == 0 {
        return false;
    }
    unsafe {
        core::ptr::addr_of_mut!(KBD).write(h.kbd_slot != 0);
        core::ptr::addr_of_mut!(MSC).write(h.msc_slot != 0);
        core::ptr::addr_of_mut!(FAT).write(h.fat);
    }
    *host_mut() = Some(h);
    true
}

/// After the well. Maps the BAR, takes the HC, looks for a boot keyboard.
pub fn init() {
    *host_mut() = None;
    unsafe {
        core::ptr::addr_of_mut!(KBD).write(false);
        core::ptr::addr_of_mut!(MSC).write(false);
        core::ptr::addr_of_mut!(FAT).write(false);
        core::ptr::addr_of_mut!(FOUND).write(false);
        core::ptr::addr_of_mut!(MISS).write(0);
    }
    let mut bars = [crate::pci::XhciBar { bar: 0, len: 0 }; 4];
    let n = crate::pci::iter_xhci(&mut bars);
    if n == 0 {
        return;
    }
    unsafe {
        core::ptr::addr_of_mut!(FOUND).write(true);
    }
    let mut i = 0usize;
    while i < n {
        if bars[i].bar != 0 && bringup(bars[i].bar, bars[i].len) {
            return;
        }
        i += 1;
    }
}

pub fn found() -> bool {
    unsafe { core::ptr::addr_of!(FOUND).read() }
}

pub fn kbd_live() -> bool {
    unsafe { core::ptr::addr_of!(KBD).read() }
}

pub fn msc_live() -> bool {
    unsafe { core::ptr::addr_of!(MSC).read() }
}

pub fn fat_live() -> bool {
    unsafe { core::ptr::addr_of!(FAT).read() }
}

/// Glass/serial word after `msc`: `fat` when LBA 0 is a FAT boot sector.
pub fn fat_line() -> &'static str {
    if fat_live() {
        "fat"
    } else {
        "no fat"
    }
}

/// Glass/serial word after `tick`: `msc` or the step that missed.
pub fn msc_line() -> &'static str {
    if msc_live() {
        return "msc";
    }
    match unsafe { core::ptr::addr_of!(MISS).read() } {
        0 => "no ccs",
        1 => "no rst",
        2 => "no addr",
        3 => "no desc",
        4 => "no bot",
        5 => "no cap",
        _ => "no msc",
    }
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
