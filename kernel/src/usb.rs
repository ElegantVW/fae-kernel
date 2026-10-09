//! xHCI + HID boot keyboard + MSC (BOT). Event ring is polled.
//! Partition walk lives in `store`; FAT is one arm of that map.

use core::cell::UnsafeCell;
use core::sync::atomic::{Ordering, compiler_fence, fence};

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

/// Saved BPB + partition LBA after `store` names a FAT arm. House names
/// stay Gleam. `vol_ok` is independent of `fat` (`looks_fat` still lights
/// the glass even when the BPB is too odd to walk).
#[derive(Clone, Copy)]
struct FatVol {
    lba: u32,
    spc: u8,
    fats: u8,
    fat16: bool,
    reserved: u16,
    fat_sz: u32,
    root_ent: u16,
    root_clus: u32,
    vol_id: u32,
    tot_sec: u32,
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
    vol_ok: bool,
    vol: FatVol,
    /// Next FAT32 cluster to try when allocating. Wraps at `max_clus`.
    alloc_rover: u32,
    /// READ CAPACITY block length and last-LBA+1. `store` paints these.
    blk: u32,
    n_lba: u32,
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
        if k != 0 && !hid_has(&h.kbd_prev, k) {
            let a = hid_ascii(k);
            if a != 0 {
                crate::kbd::push_ascii(a);
            }
        }
        i += 1;
    }
    i = 0;
    while i < 6 {
        let k = h.kbd_prev[i];
        if k != 0 && !hid_has(&now, k) {
            let a = hid_ascii(k);
            if a != 0 {
                crate::kbd::key_up(a);
            }
        }
        i += 1;
    }
    h.kbd_prev = now;
    let _ = h.intr.enq(h.report, 8, (TRB_NORMAL << 10) | IOC | ISP);
    doorbell(h, h.kbd_slot as u32, h.kbd_dci as u32);
}

fn hid_has(keys: &[u8; 6], k: u8) -> bool {
    let mut i = 0usize;
    while i < 6 {
        if keys[i] == k {
            return true;
        }
        i += 1;
    }
    false
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
    if speed >= 4 { h.st_usb3 } else { h.st_usb2 }
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
    if ty != 0 { try_ty(h, 0) } else { None }
}

fn bind_slot(h: &mut Host, slot: u8) {
    let out = page();
    w64(h.dcbaa + slot as u64 * 8, out);
    let ep0 = page();
    h.slot_ep0[slot as usize] = ring_new(ep0);
}

fn fill_slot_ctx(
    h: &Host,
    speed: u32,
    root_port: u32,
    route: u32,
    tt_slot: u32,
    tt_port: u32,
    entries: u32,
) {
    let cs = h.ctxsz as u64;
    let slot_ctx = h.in_ctx + cs;
    w32(
        slot_ctx,
        (entries << 27) | (speed << 20) | (route & 0xF_FFFF),
    );
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
    command(h, h.in_ctx, 0, (TRB_ADDRESS << 10) | ((slot as u32) << 24))
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
    command(h, h.in_ctx, 0, (TRB_EVAL << 10) | ((slot as u32) << 24))
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
    let setup_addr =
        h.slot_ep0[slot as usize].enq_held(pkt, 8, (TRB_SETUP << 10) | IDT | (trt << 16));
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
    crate::kbd::prefer_usb();
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

fn r16le(a: u64) -> u16 {
    r8(a) as u16 | (r8(a + 1) as u16) << 8
}

fn r32le(a: u64) -> u32 {
    r8(a) as u32 | (r8(a + 1) as u32) << 8 | (r8(a + 2) as u32) << 16 | (r8(a + 3) as u32) << 24
}

fn w16le(a: u64, v: u16) {
    w8(a, v as u8);
    w8(a + 1, (v >> 8) as u8);
}

fn w32le(a: u64, v: u32) {
    w8(a, v as u8);
    w8(a + 1, (v >> 8) as u8);
    w8(a + 2, (v >> 16) as u8);
    w8(a + 3, (v >> 24) as u8);
}

/// `looks_fat` lights the glass. Parse the BPB when we can so `glean` may walk.
fn take_fat(h: &mut Host, lba: u32, buf: u64) -> bool {
    if !crate::store::looks_fat(buf) {
        return false;
    }
    match parse_vol(lba, buf) {
        Some(v) => {
            h.vol = v;
            h.vol_ok = true;
        }
        None => h.vol_ok = false,
    }
    true
}

fn parse_vol(lba: u32, p: u64) -> Option<FatVol> {
    if r16le(p + 11) != 512 {
        return None;
    }
    let spc = r8(p + 13);
    if spc == 0 || (spc & (spc - 1)) != 0 {
        return None;
    }
    let reserved = r16le(p + 14);
    if reserved == 0 {
        return None;
    }
    let fats = r8(p + 16);
    if fats == 0 {
        return None;
    }
    let root_ent = r16le(p + 17);
    let fat_sz16 = r16le(p + 22);
    let fat12 = r8(p + 54) == b'F'
        && r8(p + 55) == b'A'
        && r8(p + 56) == b'T'
        && r8(p + 57) == b'1'
        && r8(p + 58) == b'2';
    let fat32 = r8(p + 82) == b'F'
        && r8(p + 83) == b'A'
        && r8(p + 84) == b'T'
        && r8(p + 85) == b'3'
        && r8(p + 86) == b'2';
    let tot16 = r16le(p + 19);
    let tot32 = r32le(p + 32);
    let tot_sec = if tot16 != 0 { tot16 as u32 } else { tot32 };
    if tot_sec == 0 {
        return None;
    }
    if fat32 {
        let fat_sz = r32le(p + 36);
        if fat_sz == 0 {
            return None;
        }
        let root_clus = r32le(p + 44);
        if root_clus < 2 {
            return None;
        }
        Some(FatVol {
            lba,
            spc,
            fats,
            fat16: false,
            reserved,
            fat_sz,
            root_ent: 0,
            root_clus,
            vol_id: r32le(p + 67),
            tot_sec,
        })
    } else if fat12 {
        None
    } else if fat_sz16 != 0 {
        Some(FatVol {
            lba,
            spc,
            fats,
            fat16: true,
            reserved,
            fat_sz: fat_sz16 as u32,
            root_ent,
            root_clus: 0,
            vol_id: r32le(p + 39),
            tot_sec,
        })
    } else {
        None
    }
}

/// One 512-byte READ(10) into the MSC data page. `store` walks with this.
pub(crate) fn read_sec(lba: u32) -> Option<u64> {
    let h = host_mut().as_mut()?;
    if h.msc_slot == 0 || h.blk != 512 {
        return None;
    }
    let buf = h.data + MSC_DATA;
    if msc_read10(h, lba, buf, 1, 512) {
        Some(buf)
    } else {
        None
    }
}

pub(crate) fn disk_geom() -> Option<(u32, u32)> {
    let h = host_mut().as_ref()?;
    if h.msc_slot == 0 || h.blk == 0 {
        return None;
    }
    Some((h.blk, h.n_lba))
}

fn claim_first_fat() -> bool {
    let Some(lba) = crate::store::first_fat() else {
        return false;
    };
    let Some(h) = host_mut().as_mut() else {
        return false;
    };
    let buf = h.data + MSC_DATA;
    if !msc_read10(h, lba, buf, 1, 512) {
        return false;
    }
    if take_fat(h, lba, buf) {
        h.fat = true;
        true
    } else {
        false
    }
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

/// WRITE(10) of `blocks`. Refuses unless this volume is KINDLING `85C7-AA81`.
fn msc_write10(h: &mut Host, lba: u32, buf: u64, blocks: u16, blk: u32) -> bool {
    if h.vol.vol_id != KINDLING_VOL {
        return false;
    }
    if blocks == 0 || blk == 0 {
        return false;
    }
    let bytes = blk.saturating_mul(blocks as u32);
    if bytes == 0 || bytes as usize > PAGE {
        return false;
    }
    let cdb = [
        0x2A,
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
    bot(h, &cdb, buf, bytes, false)
}

const GLEAN_MAX: u64 = 1 << 20;
const MAX_DIR_SEC: u32 = 256;
const MAX_FILE_SEC: u32 = 2048;
/// FAT volume serial the Databar prints as `85C7-AA81`.
const KINDLING_VOL: u32 = 0x85C7_AA81;
const MSC_VERIFY: u64 = MSC_DATA + 512;
const EPERM: u64 = 1;
const ENOENT: u64 = 2;
const EIO: u64 = 5;
const ENODEV: u64 = 19;

fn err(n: u64) -> u64 {
    0u64.wrapping_sub(n)
}

#[derive(Clone, Copy)]
struct FatHit {
    clus: u32,
    size: u32,
    dir_lba: u32,
    dir_off: u16,
}

enum Scan {
    Found(FatHit),
    End,
    More,
}

struct LfnAcc {
    buf: [u8; 64],
    len: usize,
    ck: u8,
    expect: u8,
    ready: bool,
}

impl LfnAcc {
    fn new() -> Self {
        Self {
            buf: [0; 64],
            len: 0,
            ck: 0,
            expect: 0,
            ready: false,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.ck = 0;
        self.expect = 0;
        self.ready = false;
        let mut i = 0usize;
        while i < 64 {
            self.buf[i] = 0;
            i += 1;
        }
    }
}

/// Byte length of a named root file, or `-errno`. Empty is `0`.
pub fn fat_file_len(name: &[u8]) -> u64 {
    if name.is_empty() || name.len() > 64 {
        return err(EPERM);
    }
    let Some(h) = host_mut().as_mut() else {
        return err(ENODEV);
    };
    if h.msc_slot == 0 || !h.fat {
        return err(ENODEV);
    }
    if !h.vol_ok {
        return err(EIO);
    }
    match find_root(h, name) {
        Ok(hit) => hit.size as u64,
        Err(e) => err(e),
    }
}

/// Exact load of a named root file into `buf`. `cap` is the probed size
/// (1..=64 KiB). DMA dest stays MSC_DATA. Short or long is `-EIO`.
pub fn load_fat(name: &[u8], buf: u64, cap: u64) -> u64 {
    if buf == 0 || cap == 0 || cap > 65536 || name.is_empty() || name.len() > 64 {
        return err(EPERM);
    }
    let Some(h) = host_mut().as_mut() else {
        return err(ENODEV);
    };
    if h.msc_slot == 0 || !h.fat {
        return err(ENODEV);
    }
    if !h.vol_ok {
        return err(EIO);
    }
    match find_root(h, name) {
        Ok(hit) => {
            if hit.size as u64 != cap {
                return err(EIO);
            }
            let n = read_chain(h, hit.clus, hit.size, buf, cap);
            if n != cap {
                if (n as i64) < 0 {
                    return n;
                }
                return err(EIO);
            }
            n
        }
        Err(e) => err(e),
    }
}

/// Gather a named leaf from the FAT volume's root. Gleam name (1–64), not 8.3.
/// FAT is the medium. Cap 1 MiB. Read only. DMA stays on the proven MSC page.
pub fn glean_fat(name: &[u8], buf: u64, len: u64) -> u64 {
    if buf == 0 || len == 0 || len > GLEAN_MAX || name.is_empty() || name.len() > 64 {
        return err(EPERM);
    }
    let Some(h) = host_mut().as_mut() else {
        return err(ENODEV);
    };
    if h.msc_slot == 0 || !h.fat {
        return err(ENODEV);
    }
    if !h.vol_ok {
        return err(EIO);
    }
    match find_root(h, name) {
        Ok(hit) => read_chain(h, hit.clus, hit.size, buf, len),
        Err(e) => err(e),
    }
}

fn find_root(h: &mut Host, name: &[u8]) -> Result<FatHit, u64> {
    let vol = h.vol;
    if vol.fat16 {
        find_root16(h, &vol, name)
    } else {
        find_root32(h, &vol, name)
    }
}

fn root16_secs(vol: &FatVol) -> u32 {
    ((vol.root_ent as u32).saturating_mul(32).saturating_add(511)) / 512
}

fn fats_secs(vol: &FatVol) -> u32 {
    (vol.fats as u32).saturating_mul(vol.fat_sz)
}

fn clus_lba(vol: &FatVol, clus: u32) -> u32 {
    let root = if vol.fat16 { root16_secs(vol) } else { 0 };
    vol.lba
        .wrapping_add(vol.reserved as u32)
        .wrapping_add(fats_secs(vol))
        .wrapping_add(root)
        .wrapping_add(clus.wrapping_sub(2).wrapping_mul(vol.spc as u32))
}

fn find_root16(h: &mut Host, vol: &FatVol, name: &[u8]) -> Result<FatHit, u64> {
    let secs = root16_secs(vol);
    if secs == 0 {
        return Err(ENOENT);
    }
    let start = vol
        .lba
        .wrapping_add(vol.reserved as u32)
        .wrapping_add(fats_secs(vol));
    let mut lfn = LfnAcc::new();
    let mut short = None;
    let mut k = 0u32;
    while k < secs {
        if k >= MAX_DIR_SEC {
            return Err(EIO);
        }
        let lba = start.wrapping_add(k);
        if !msc_read10(h, lba, h.data + MSC_DATA, 1, 512) {
            return Err(EIO);
        }
        match scan_sec(h.data + MSC_DATA, lba, name, &mut lfn, &mut short) {
            Scan::Found(hit) => return Ok(hit),
            Scan::End => break,
            Scan::More => {}
        }
        k = k.saturating_add(1);
    }
    short.ok_or(ENOENT)
}

fn find_root32(h: &mut Host, vol: &FatVol, name: &[u8]) -> Result<FatHit, u64> {
    let mut clus = vol.root_clus;
    let mut secs = 0u32;
    let mut lfn = LfnAcc::new();
    let mut short = None;
    loop {
        if clus < 2 {
            return Err(EIO);
        }
        let base = clus_lba(vol, clus);
        let mut s = 0u8;
        while s < vol.spc {
            if secs >= MAX_DIR_SEC {
                return Err(EIO);
            }
            let lba = base.wrapping_add(s as u32);
            if !msc_read10(h, lba, h.data + MSC_DATA, 1, 512) {
                return Err(EIO);
            }
            match scan_sec(h.data + MSC_DATA, lba, name, &mut lfn, &mut short) {
                Scan::Found(hit) => return Ok(hit),
                Scan::End => return short.ok_or(ENOENT),
                Scan::More => {}
            }
            secs = secs.saturating_add(1);
            s = s.saturating_add(1);
        }
        match fat_next(h, vol, clus) {
            None => return Err(EIO),
            Some(0) => return short.ok_or(ENOENT),
            Some(n) => clus = n,
        }
    }
}

fn scan_sec(sec: u64, lba: u32, name: &[u8], lfn: &mut LfnAcc, short: &mut Option<FatHit>) -> Scan {
    let mut e = 0u64;
    while e < 512 {
        let ent = sec + e;
        let first = r8(ent);
        if first == 0 {
            return Scan::End;
        }
        let attr = r8(ent + 11);
        if first == 0xE5 {
            lfn.clear();
        } else if attr == 0x0F {
            lfn_feed(lfn, ent);
        } else if attr & 0x18 != 0 {
            lfn.clear();
        } else {
            match match_file(ent, name, lfn) {
                Hit::Lfn => {
                    return Scan::Found(hit_from_ent(ent, lba, e as u16));
                }
                Hit::Short => {
                    if short.is_none() {
                        *short = Some(hit_from_ent(ent, lba, e as u16));
                    }
                    lfn.clear();
                }
                Hit::None => lfn.clear(),
            }
        }
        e += 32;
    }
    Scan::More
}

fn hit_from_ent(ent: u64, lba: u32, dir_off: u16) -> FatHit {
    FatHit {
        clus: (r16le(ent + 20) as u32) << 16 | r16le(ent + 26) as u32,
        size: r32le(ent + 28),
        dir_lba: lba,
        dir_off,
    }
}

fn fold_ascii(b: u8) -> u8 {
    if (b'A'..=b'Z').contains(&b) {
        b + 32
    } else {
        b
    }
}

fn name_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0usize;
    while i < a.len() {
        if fold_ascii(a[i]) != fold_ascii(b[i]) {
            return false;
        }
        i += 1;
    }
    true
}

enum Hit {
    /// Long name matches the Gleam name, byte for byte.
    Lfn,
    /// No LFN; 8.3 alias matches when ASCII case is folded.
    Short,
    None,
}

fn match_file(ent: u64, name: &[u8], lfn: &LfnAcc) -> Hit {
    if lfn.ready && lfn.ck == short_cksum(ent) {
        if lfn.len == name.len() && &lfn.buf[..lfn.len] == name {
            return Hit::Lfn;
        }
        return Hit::None;
    }
    let mut short = [0u8; 64];
    let n = eight_three(ent, &mut short);
    if n == name.len() && name_eq(&short[..n], name) {
        Hit::Short
    } else {
        Hit::None
    }
}

fn eight_three(ent: u64, out: &mut [u8; 64]) -> usize {
    let mut n = 0usize;
    let mut end = 8u64;
    while end > 0 && r8(ent + end - 1) == b' ' {
        end -= 1;
    }
    let mut i = 0u64;
    while i < end && n < 64 {
        let b = r8(ent + i);
        if !(0x20..=0x7E).contains(&b) {
            return 0;
        }
        out[n] = b;
        n += 1;
        i += 1;
    }
    let mut eend = 11u64;
    while eend > 8 && r8(ent + eend - 1) == b' ' {
        eend -= 1;
    }
    if eend > 8 {
        if n >= 64 {
            return 0;
        }
        out[n] = b'.';
        n += 1;
        i = 8;
        while i < eend && n < 64 {
            let b = r8(ent + i);
            if !(0x20..=0x7E).contains(&b) {
                return 0;
            }
            out[n] = b;
            n += 1;
            i += 1;
        }
    }
    n
}

fn short_cksum(ent: u64) -> u8 {
    let mut sum = 0u8;
    let mut i = 0u64;
    while i < 11 {
        sum = ((sum & 1) << 7)
            .wrapping_add(sum >> 1)
            .wrapping_add(r8(ent + i));
        i += 1;
    }
    sum
}

fn lfn_feed(lfn: &mut LfnAcc, ent: u64) {
    let seq = r8(ent);
    if seq == 0 || seq == 0xE5 {
        lfn.clear();
        return;
    }
    let last = seq & 0x40 != 0;
    let ord = seq & 0x1F;
    if ord == 0 {
        lfn.clear();
        return;
    }
    if last {
        lfn.clear();
        lfn.expect = ord;
        lfn.ck = r8(ent + 13);
    }
    if lfn.expect == 0 || ord != lfn.expect {
        lfn.clear();
        return;
    }
    let off = (ord as usize - 1) * 13;
    if !lfn_place(ent, &mut lfn.buf, off) {
        lfn.clear();
        return;
    }
    lfn.expect -= 1;
    if lfn.expect == 0 {
        let mut n = 0usize;
        while n < 64 && lfn.buf[n] != 0 {
            n += 1;
        }
        if n == 0 {
            lfn.clear();
            return;
        }
        lfn.len = n;
        lfn.ready = true;
    }
}

fn lfn_place(ent: u64, buf: &mut [u8; 64], off: usize) -> bool {
    const POS: [u64; 13] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
    let mut i = 0usize;
    while i < 13 {
        let at = off + i;
        let lo = r8(ent + POS[i]);
        let hi = r8(ent + POS[i] + 1);
        let ch = lo as u16 | (hi as u16) << 8;
        if ch == 0 || ch == 0xFFFF {
            if at < 64 {
                buf[at] = 0;
            }
            return true;
        }
        if hi != 0 || !(0x20..=0x7E).contains(&lo) {
            return false;
        }
        if at >= 64 {
            return false;
        }
        buf[at] = lo;
        i += 1;
    }
    true
}

fn fat_next(h: &mut Host, vol: &FatVol, clus: u32) -> Option<u32> {
    let fat0 = vol.lba.wrapping_add(vol.reserved as u32);
    if vol.fat16 {
        let off = clus.saturating_mul(2);
        let sec = fat0.wrapping_add(off / 512);
        let ent = (off % 512) as u64;
        if !msc_read10(h, sec, h.data + MSC_DATA, 1, 512) {
            return None;
        }
        let v = r16le(h.data + MSC_DATA + ent) as u32;
        if v >= 0xFFF8 {
            return Some(0);
        }
        if v == 0xFFF7 || v < 2 {
            return None;
        }
        Some(v)
    } else {
        let off = clus.saturating_mul(4);
        let sec = fat0.wrapping_add(off / 512);
        let ent = (off % 512) as u64;
        if !msc_read10(h, sec, h.data + MSC_DATA, 1, 512) {
            return None;
        }
        let v = r32le(h.data + MSC_DATA + ent) & 0x0FFF_FFFF;
        if v >= 0x0FFF_FFF8 {
            return Some(0);
        }
        if v == 0x0FFF_FFF7 || v < 2 {
            return None;
        }
        Some(v)
    }
}

fn read_chain(h: &mut Host, mut clus: u32, size: u32, buf: u64, len: u64) -> u64 {
    let n = len.min(size as u64).min(GLEAN_MAX);
    if n == 0 {
        return 0;
    }
    if clus < 2 {
        return err(EIO);
    }
    let vol = h.vol;
    let mut copied = 0u64;
    let mut secs = 0u32;
    while copied < n {
        if clus < 2 {
            return err(EIO);
        }
        let base = clus_lba(&vol, clus);
        let mut s = 0u8;
        while s < vol.spc && copied < n {
            if secs >= MAX_FILE_SEC {
                break;
            }
            if !msc_read10(h, base.wrapping_add(s as u32), h.data + MSC_DATA, 1, 512) {
                return err(EIO);
            }
            let take = (n - copied).min(512);
            let mut i = 0u64;
            while i < take {
                let b = r8(h.data + MSC_DATA + i);
                unsafe {
                    ((buf + copied + i) as *mut u8).write_volatile(b);
                }
                i += 1;
            }
            copied += take;
            secs = secs.saturating_add(1);
            s = s.saturating_add(1);
        }
        if copied >= n {
            break;
        }
        match fat_next(h, &vol, clus) {
            None => return err(EIO),
            Some(0) => break,
            Some(next) => clus = next,
        }
    }
    copied
}

fn write_chain(h: &mut Host, mut clus: u32, size: u32, buf: u64) -> u64 {
    let n = size as u64;
    if n == 0 || n > GLEAN_MAX {
        return err(EPERM);
    }
    if h.vol.vol_id != KINDLING_VOL {
        return err(EPERM);
    }
    if clus < 2 {
        return err(EIO);
    }
    let vol = h.vol;
    let mut copied = 0u64;
    let mut secs = 0u32;
    while copied < n {
        if clus < 2 {
            return err(EIO);
        }
        let base = clus_lba(&vol, clus);
        let mut s = 0u8;
        while s < vol.spc && copied < n {
            if secs >= MAX_FILE_SEC {
                return err(EIO);
            }
            let lba = base.wrapping_add(s as u32);
            if !msc_read10(h, lba, h.data + MSC_DATA, 1, 512) {
                return err(EIO);
            }
            let take = (n - copied).min(512);
            let mut i = 0u64;
            while i < take {
                let b = unsafe { ((buf + copied + i) as *const u8).read_volatile() };
                w8(h.data + MSC_DATA + i, b);
                i += 1;
            }
            let mut k = 0u64;
            while k < 512 {
                w8(h.data + MSC_VERIFY + k, r8(h.data + MSC_DATA + k));
                k += 1;
            }
            if !commit_prepared(h, lba) {
                return err(EIO);
            }
            copied += take;
            secs = secs.saturating_add(1);
            s = s.saturating_add(1);
        }
        if copied >= n {
            break;
        }
        match fat_next(h, &vol, clus) {
            None => return err(EIO),
            Some(0) => return err(EIO),
            Some(next) => clus = next,
        }
    }
    copied
}

fn speak_fat(s: &'static str) {
    serial_print(s);
    crate::glass::put_bytes(s.as_ptr(), s.len() as u64);
}

const FAT_EOC: u32 = 0x0FFF_FFFF;

fn sector_match(h: &Host) -> bool {
    let mut k = 0u64;
    while k < 512 {
        if r8(h.data + MSC_DATA + k) != r8(h.data + MSC_VERIFY + k) {
            return false;
        }
        k += 1;
    }
    true
}

fn restore_verify(h: &mut Host) {
    let mut k = 0u64;
    while k < 512 {
        w8(h.data + MSC_DATA + k, r8(h.data + MSC_VERIFY + k));
        k += 1;
    }
}

/// WRITE(10) the sector already copied into MSC_VERIFY; READ(10) compare.
/// Three tries. TUR + settle between misses — iron flash can lag the cache.
fn commit_prepared(h: &mut Host, lba: u32) -> bool {
    let mut tries = 0u8;
    while tries < 3 {
        restore_verify(h);
        if msc_write10(h, lba, h.data + MSC_DATA, 1, 512)
            && msc_read10(h, lba, h.data + MSC_DATA, 1, 512)
            && sector_match(h)
        {
            return true;
        }
        let _ = msc_ready(h);
        recover();
        tries = tries.saturating_add(1);
    }
    false
}

fn commit_sec(h: &mut Host, lba: u32) -> bool {
    let mut k = 0u64;
    while k < 512 {
        w8(h.data + MSC_VERIFY + k, r8(h.data + MSC_DATA + k));
        k += 1;
    }
    commit_prepared(h, lba)
}

fn max_clus(vol: &FatVol) -> u32 {
    let fat_secs = fats_secs(vol);
    let root = if vol.fat16 { root16_secs(vol) } else { 0 };
    let data = vol
        .tot_sec
        .saturating_sub(vol.reserved as u32)
        .saturating_sub(fat_secs)
        .saturating_sub(root);
    2 + data / (vol.spc as u32)
}

fn clusters_for(vol: &FatVol, len: u64) -> u32 {
    let bpc = vol.spc as u32 * 512;
    if bpc == 0 {
        return 0;
    }
    let n = ((len as u32).saturating_add(bpc - 1)) / bpc;
    if n == 0 { 1 } else { n }
}

fn fat_get(h: &mut Host, clus: u32) -> Option<u32> {
    if h.vol.fat16 {
        return None;
    }
    let off = clus.saturating_mul(4);
    let lba = h
        .vol
        .lba
        .wrapping_add(h.vol.reserved as u32)
        .wrapping_add(off / 512);
    let ent = (off % 512) as u64;
    if !msc_read10(h, lba, h.data + MSC_DATA, 1, 512) {
        return None;
    }
    Some(r32le(h.data + MSC_DATA + ent) & 0x0FFF_FFFF)
}

fn fat_set(h: &mut Host, clus: u32, val: u32) -> bool {
    if h.vol.fat16 {
        return false;
    }
    let vol = h.vol;
    let off = clus.saturating_mul(4);
    let sec_off = off / 512;
    let ent = (off % 512) as u64;
    let mut fi = 0u8;
    while fi < vol.fats {
        let lba = vol
            .lba
            .wrapping_add(vol.reserved as u32)
            .wrapping_add(fi as u32 * vol.fat_sz)
            .wrapping_add(sec_off);
        if !msc_read10(h, lba, h.data + MSC_DATA, 1, 512) {
            return false;
        }
        let old = r32le(h.data + MSC_DATA + ent);
        let new = (old & 0xF000_0000) | (val & 0x0FFF_FFFF);
        w32le(h.data + MSC_DATA + ent, new);
        if !commit_sec(h, lba) {
            return false;
        }
        fi = fi.saturating_add(1);
    }
    true
}

fn alloc_scan(h: &mut Host, from: u32, to: u32) -> Option<u32> {
    if h.vol.fat16 || from >= to {
        return None;
    }
    let fat0 = h.vol.lba.wrapping_add(h.vol.reserved as u32);
    let mut clus = from;
    while clus < to {
        let sec_i = clus / 128;
        let lba = fat0.wrapping_add(sec_i);
        if !msc_read10(h, lba, h.data + MSC_DATA, 1, 512) {
            return None;
        }
        let sec_first = sec_i.saturating_mul(128);
        let sec_last = sec_first.saturating_add(128);
        let end = if to < sec_last { to } else { sec_last };
        let mut e_clus = clus;
        while e_clus < end {
            let ent = (e_clus - sec_first) * 4;
            let v = r32le(h.data + MSC_DATA + ent as u64) & 0x0FFF_FFFF;
            if e_clus >= 2 && v == 0 {
                if fat_set(h, e_clus, FAT_EOC) {
                    h.alloc_rover = e_clus.saturating_add(1);
                    return Some(e_clus);
                }
                return None;
            }
            e_clus = e_clus.saturating_add(1);
        }
        clus = end;
    }
    None
}

fn alloc_one(h: &mut Host) -> Option<u32> {
    let max = max_clus(&h.vol);
    if max <= 2 {
        return None;
    }
    let start = if h.alloc_rover >= 2 && h.alloc_rover < max {
        h.alloc_rover
    } else {
        2
    };
    if let Some(c) = alloc_scan(h, start, max) {
        return Some(c);
    }
    if start > 2 {
        alloc_scan(h, 2, start)
    } else {
        None
    }
}

fn free_chain(h: &mut Host, first: u32) {
    let mut c = first;
    let mut n = 0u32;
    while c >= 2 && n < MAX_FILE_SEC {
        let next = match fat_get(h, c) {
            Some(v) if v >= 0x0FFF_FFF8 => 0,
            Some(v) if v >= 2 => v,
            _ => 0,
        };
        let _ = fat_set(h, c, 0);
        if next == 0 {
            break;
        }
        c = next;
        n = n.saturating_add(1);
    }
}

fn alloc_chain(h: &mut Host, nclus: u32) -> Option<u32> {
    if nclus == 0 || nclus > MAX_FILE_SEC {
        return None;
    }
    let first = alloc_one(h)?;
    let mut prev = first;
    let mut i = 1u32;
    while i < nclus {
        let Some(c) = alloc_one(h) else {
            free_chain(h, first);
            return None;
        };
        if !fat_set(h, prev, c) {
            free_chain(h, first);
            return None;
        }
        prev = c;
        i = i.saturating_add(1);
    }
    if !fat_set(h, prev, FAT_EOC) {
        free_chain(h, first);
        return None;
    }
    Some(first)
}

fn zero_chain(h: &mut Host, first: u32) -> bool {
    let mut c = first;
    let mut n = 0u32;
    while c >= 2 && n < MAX_FILE_SEC {
        if !zero_clus(h, c) {
            return false;
        }
        let vol = h.vol;
        match fat_next(h, &vol, c) {
            Some(0) => return true,
            Some(next) => c = next,
            None => return false,
        }
        n = n.saturating_add(1);
    }
    false
}

fn zero_clus(h: &mut Host, clus: u32) -> bool {
    if clus < 2 {
        return false;
    }
    let base = clus_lba(&h.vol, clus);
    let mut s = 0u8;
    while s < h.vol.spc {
        let mut i = 0u64;
        while i < 512 {
            w8(h.data + MSC_DATA + i, 0);
            i += 1;
        }
        if !commit_sec(h, base.wrapping_add(s as u32)) {
            return false;
        }
        s = s.saturating_add(1);
    }
    true
}

fn ascii_upper(b: u8) -> u8 {
    if (b'a'..=b'z').contains(&b) {
        b - 32
    } else {
        b
    }
}

fn is_83_char(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'!'
            | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'-'
            | b'@'
            | b'^'
            | b'_'
            | b'`'
            | b'{'
            | b'}'
            | b'~'
    )
}

fn make_83(name: &[u8], out: &mut [u8; 11]) {
    let mut i = 0usize;
    while i < 11 {
        out[i] = b' ';
        i += 1;
    }
    let mut ok = !name.is_empty() && name.len() <= 8;
    i = 0;
    while i < name.len() {
        if name[i] == b'.' || !is_83_char(name[i]) {
            ok = false;
            break;
        }
        i += 1;
    }
    if ok {
        i = 0;
        while i < name.len() {
            out[i] = ascii_upper(name[i]);
            i += 1;
        }
        return;
    }
    let mut n = 0usize;
    i = 0;
    while i < name.len() && n < 6 {
        let b = name[i];
        if is_83_char(b) && b != b'.' {
            out[n] = ascii_upper(b);
            n += 1;
        }
        i += 1;
    }
    if n == 0 {
        out[0] = b'X';
        n = 1;
    }
    out[n] = b'~';
    out[n + 1] = b'1';
}

fn short_cksum11(n: &[u8; 11]) -> u8 {
    let mut sum = 0u8;
    let mut i = 0usize;
    while i < 11 {
        sum = ((sum & 1) << 7).wrapping_add(sum >> 1).wrapping_add(n[i]);
        i += 1;
    }
    sum
}

fn lfn_count(name: &[u8]) -> usize {
    (name.len() + 12) / 13
}

fn put_ent(h: &mut Host, lba: u32, off: u16, ent: &[u8; 32]) -> bool {
    if (off as u64) > 480 {
        return false;
    }
    if !msc_read10(h, lba, h.data + MSC_DATA, 1, 512) {
        return false;
    }
    let mut i = 0usize;
    while i < 32 {
        w8(h.data + MSC_DATA + off as u64 + i as u64, ent[i]);
        i += 1;
    }
    commit_sec(h, lba)
}

fn fill_lfn(dst: &mut [u8; 32], seq: u8, ck: u8, name: &[u8], chunk_off: usize) {
    let mut i = 0usize;
    while i < 32 {
        dst[i] = 0;
        i += 1;
    }
    dst[0] = seq;
    dst[11] = 0x0F;
    dst[13] = ck;
    const POS: [usize; 13] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
    i = 0;
    while i < 13 {
        let ni = chunk_off + i;
        let (lo, hi) = if ni < name.len() {
            (name[ni], 0u8)
        } else if ni == name.len() {
            (0, 0)
        } else {
            (0xFF, 0xFF)
        };
        dst[POS[i]] = lo;
        dst[POS[i] + 1] = hi;
        i += 1;
    }
}

fn fill_83(dst: &mut [u8; 32], name11: &[u8; 11], clus: u32, size: u32) {
    let mut i = 0usize;
    while i < 32 {
        dst[i] = 0;
        i += 1;
    }
    i = 0;
    while i < 11 {
        dst[i] = name11[i];
        i += 1;
    }
    dst[11] = 0x20;
    dst[26] = clus as u8;
    dst[27] = (clus >> 8) as u8;
    dst[20] = (clus >> 16) as u8;
    dst[21] = (clus >> 24) as u8;
    dst[28] = size as u8;
    dst[29] = (size >> 8) as u8;
    dst[30] = (size >> 16) as u8;
    dst[31] = (size >> 24) as u8;
}

fn dir_last_clus(h: &mut Host) -> Option<u32> {
    let mut c = h.vol.root_clus;
    let mut n = 0u32;
    loop {
        if c < 2 {
            return None;
        }
        let vol = h.vol;
        match fat_next(h, &vol, c) {
            None => return None,
            Some(0) => return Some(c),
            Some(next) => {
                c = next;
                n = n.saturating_add(1);
                if n >= MAX_DIR_SEC {
                    return None;
                }
            }
        }
    }
}

fn dir_extend(h: &mut Host) -> Option<u32> {
    let last = dir_last_clus(h)?;
    let c = alloc_one(h)?;
    if !fat_set(h, last, c) || !fat_set(h, c, FAT_EOC) || !zero_clus(h, c) {
        let _ = fat_set(h, c, 0);
        return None;
    }
    Some(c)
}

fn take_slot(slots: &mut [(u32, u16); 8], run: usize, lba: u32, off: u16) {
    if run < 8 {
        slots[run] = (lba, off);
    }
}

/// Grow the root and append free slots onto an existing run (0xE5 tail or
/// a 0 terminator that did not leave enough room in this cluster).
fn extend_dir_slots(
    h: &mut Host,
    need: usize,
    mut run: usize,
    slots: &mut [(u32, u16); 8],
) -> bool {
    while run < need {
        let Some(newc) = dir_extend(h) else {
            return false;
        };
        let base = clus_lba(&h.vol, newc);
        let mut s = 0u8;
        while run < need && s < h.vol.spc {
            let lba = base.wrapping_add(s as u32);
            let mut off = 0u16;
            while run < need && off <= 480 {
                take_slot(slots, run, lba, off);
                run += 1;
                off = off.saturating_add(32);
            }
            s = s.saturating_add(1);
        }
    }
    run == need
}

fn dir_find_slots(h: &mut Host, need: usize, slots: &mut [(u32, u16); 8]) -> bool {
    if need == 0 || need > 8 {
        return false;
    }
    let mut clus = h.vol.root_clus;
    let mut secs = 0u32;
    let mut run = 0usize;
    loop {
        if clus < 2 {
            return false;
        }
        let base = clus_lba(&h.vol, clus);
        let mut s = 0u8;
        while s < h.vol.spc {
            if secs >= MAX_DIR_SEC {
                return false;
            }
            let lba = base.wrapping_add(s as u32);
            if !msc_read10(h, lba, h.data + MSC_DATA, 1, 512) {
                return false;
            }
            let mut e = 0u64;
            while e < 512 {
                let first = r8(h.data + MSC_DATA + e);
                if first == 0 {
                    let mut off = e as u16;
                    let mut cur = lba;
                    let mut ns = s;
                    while run < need && off <= 480 {
                        take_slot(slots, run, cur, off);
                        run += 1;
                        off = off.saturating_add(32);
                    }
                    ns = ns.saturating_add(1);
                    while run < need && ns < h.vol.spc {
                        cur = base.wrapping_add(ns as u32);
                        off = 0;
                        while run < need && off <= 480 {
                            take_slot(slots, run, cur, off);
                            run += 1;
                            off = off.saturating_add(32);
                        }
                        ns = ns.saturating_add(1);
                    }
                    if run == need {
                        return true;
                    }
                    return extend_dir_slots(h, need, run, slots);
                }
                if first == 0xE5 {
                    take_slot(slots, run, lba, e as u16);
                    run += 1;
                    if run == need {
                        return true;
                    }
                } else {
                    run = 0;
                }
                e += 32;
            }
            secs = secs.saturating_add(1);
            s = s.saturating_add(1);
        }
        let vol = h.vol;
        match fat_next(h, &vol, clus) {
            None => return false,
            Some(0) => return extend_dir_slots(h, need, run, slots),
            Some(n) => clus = n,
        }
    }
}

fn dir_insert(h: &mut Host, name: &[u8], clus: u32, size: u32) -> Result<(), u64> {
    let nlfn = lfn_count(name);
    if nlfn == 0 || nlfn > 7 {
        return Err(EPERM);
    }
    let need = nlfn + 1;
    let mut slots = [(0u32, 0u16); 8];
    if !dir_find_slots(h, need, &mut slots) {
        return Err(EIO);
    }
    let mut name11 = [b' '; 11];
    make_83(name, &mut name11);
    let ck = short_cksum11(&name11);
    let mut ent = [0u8; 32];
    let mut k = 0usize;
    while k < nlfn {
        let ord = nlfn - k;
        let seq = if k == 0 {
            (ord as u8) | 0x40
        } else {
            ord as u8
        };
        fill_lfn(&mut ent, seq, ck, name, (ord - 1) * 13);
        let (lba, off) = slots[k];
        if !put_ent(h, lba, off, &ent) {
            return Err(EIO);
        }
        k += 1;
    }
    fill_83(&mut ent, &name11, clus, size);
    let (lba, off) = slots[nlfn];
    if !put_ent(h, lba, off, &ent) {
        return Err(EIO);
    }
    Ok(())
}

fn patch_dirent(h: &mut Host, hit: &FatHit, clus: u32, size: u32) -> bool {
    if !msc_read10(h, hit.dir_lba, h.data + MSC_DATA, 1, 512) {
        return false;
    }
    let e = h.data + MSC_DATA + hit.dir_off as u64;
    w16le(e + 26, clus as u16);
    w16le(e + 20, (clus >> 16) as u16);
    w32le(e + 28, size);
    commit_sec(h, hit.dir_lba)
}

fn create_file(h: &mut Host, name: &[u8], len: u64) -> Result<u32, u64> {
    recover();
    if h.vol.fat16 {
        return Err(EPERM);
    }
    let nclus = clusters_for(&h.vol, len);
    if nclus == 0 {
        return Err(EPERM);
    }
    let Some(clus) = alloc_chain(h, nclus) else {
        return Err(EIO);
    };
    if !zero_chain(h, clus) {
        free_chain(h, clus);
        return Err(EIO);
    }
    match dir_insert(h, name, clus, len as u32) {
        Ok(()) => Ok(clus),
        Err(e) => {
            free_chain(h, clus);
            Err(e)
        }
    }
}

fn finish_write(h: &mut Host, clus: u32, len: u64, buf: u64) -> u64 {
    let n = write_chain(h, clus, len as u32, buf);
    if n != len {
        if (n as i64) < 0 {
            return n;
        }
        return err(EIO);
    }
    n
}

/// Re-ink a named root file, or create it when missing / empty.
/// Exact measure. KINDLING `85C7-AA81` only. WRITE(10) each sector,
/// READ(10) compare. DMA dest stays MSC_DATA. FAT32 create; FAT16 re-inks.
pub fn stow_fat(name: &[u8], buf: u64, len: u64) -> u64 {
    if buf == 0 || len == 0 || len > GLEAN_MAX || name.is_empty() || name.len() > 64 {
        return err(EPERM);
    }
    let Some(h) = host_mut().as_mut() else {
        return err(ENODEV);
    };
    if h.msc_slot == 0 || !h.fat {
        return err(ENODEV);
    }
    if !h.vol_ok {
        return err(EIO);
    }
    if h.vol.vol_id != KINDLING_VOL {
        return err(EPERM);
    }
    match find_root(h, name) {
        Ok(hit) if hit.size as u64 == len && hit.clus >= 2 => finish_write(h, hit.clus, len, buf),
        Ok(hit) if hit.size == 0 && !h.vol.fat16 => {
            let nclus = clusters_for(&h.vol, len);
            if nclus == 0 {
                return err(EPERM);
            }
            let Some(clus) = alloc_chain(h, nclus) else {
                return err(EIO);
            };
            if !zero_chain(h, clus) {
                free_chain(h, clus);
                return err(EIO);
            }
            if !patch_dirent(h, &hit, clus, len as u32) {
                free_chain(h, clus);
                return err(EIO);
            }
            recover();
            finish_write(h, clus, len, buf)
        }
        Ok(hit) if hit.size as u64 != len => {
            speak_fat("the leaf is the wrong measure\n");
            err(EPERM)
        }
        Ok(hit) => finish_write(h, hit.clus, len, buf),
        Err(e) if e == ENOENT && !h.vol.fat16 => match create_file(h, name, len) {
            Ok(clus) => {
                recover();
                finish_write(h, clus, len, buf)
            }
            Err(e) => err(e),
        },
        Err(e) => err(e),
    }
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
        h, slot, root_port, speed, route, tt_slot, tt_port, out_ep, in_ep, out_max, in_max,
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
    let last = (r8(h.data + MSC_DATA) as u32) << 24
        | (r8(h.data + MSC_DATA + 1) as u32) << 16
        | (r8(h.data + MSC_DATA + 2) as u32) << 8
        | r8(h.data + MSC_DATA + 3) as u32;
    let blk = (r8(h.data + MSC_DATA + 4) as u32) << 24
        | (r8(h.data + MSC_DATA + 5) as u32) << 16
        | (r8(h.data + MSC_DATA + 6) as u32) << 8
        | r8(h.data + MSC_DATA + 7) as u32;
    h.blk = blk;
    h.n_lba = last.saturating_add(1);
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
    command(h, h.in_ctx, 0, (TRB_EVAL << 10) | ((slot as u32) << 24))
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

fn try_hub(h: &mut Host, hub_slot: u8, root_port: u32, hub_speed: u32) -> bool {
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
    let ss_port =
        (port as usize) < PORTS && h.st_usb3 != 0 && h.port_st[port as usize] == h.st_usb3 as u8;
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
        vol_ok: false,
        vol: FatVol {
            lba: 0,
            spc: 0,
            fats: 0,
            fat16: false,
            reserved: 0,
            fat_sz: 0,
            root_ent: 0,
            root_clus: 0,
            vol_id: 0,
            tot_sec: 0,
        },
        alloc_rover: 2,
        blk: 0,
        n_lba: 0,
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
        core::ptr::addr_of_mut!(FAT).write(false);
    }
    if h.kbd_slot != 0 {
        crate::kbd::prefer_usb();
    }
    *host_mut() = Some(h);
    if unsafe { core::ptr::addr_of!(MSC).read() } {
        crate::store::probe();
        let fat = claim_first_fat();
        unsafe {
            core::ptr::addr_of_mut!(FAT).write(fat);
        }
    }
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

/// BPB volume id when a FAT volume is live, else 0 (cairn-only book).
pub fn volume_id() -> u32 {
    match host_mut() {
        Some(h) if h.msc_slot != 0 && h.fat && h.vol_ok => h.vol.vol_id,
        _ => 0,
    }
}

/// Glass/serial word after the store dump: `fat` when a FAT arm was claimed.
pub fn fat_line() -> &'static str {
    if fat_live() { "fat" } else { "no fat" }
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
