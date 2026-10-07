//! Kindling's glass — VGA text on the paved path, GOP blit on EFI.
//!
//! Kindling flame first (Lilac 13), then the Grove clearing (Violet 5) and
//! the title `Grove`. Canonical Grove sigil: `faeOS/assets/sigils/grove.txt`.
//! Serial ceremony stays egg-free; this is the picture.

const VGA: u64 = 0xB8000;
const COLS: u16 = 80;
const ROWS: u16 = 25;
const ATTR_LILAC: u8 = 0x0D;
const ATTR_VIOLET: u8 = 0x05;
const ATTR_PARCHMENT: u8 = 0x07;

const LILAC: u32 = 0x00D4_B4E8;
const VIOLET: u32 = 0x00B0_8CC8;
const PARCHMENT: u32 = 0x00F0_E4EE;
const NIGHT: u32 = 0x001A_1218;
/// GOP log and Grove share this left margin so ingle sits under the title.
const GOP_X: u64 = 8;

/// Kindling flame — `docs/identity/logo.txt` (firmware already laid this on VGA).
const FLAME: &str = "  *\n /|\\\n/ | \\\n  |\n / \\\n/   \\\n";
/// Grove clearing — `~/faeOS/assets/sigils/grove.txt` (Violet). Embedded ink,
/// not a forked living doc.
pub(crate) const GROVE: &str = "\\  |  /\n \\| |/ \n---+---\n / | \\ \n/  |  \\\n";
pub(crate) const TITLE: &str = "Grove";

static FONT: &[u8] = include_bytes!("../../fw/font8x16.bin");

#[derive(Clone, Copy)]
struct Fb {
    addr: u64,
    width: u64,
    height: u64,
    pitch: u64,
    bpp: u16,
    bgr: bool,
}

static mut FB: Fb = Fb {
    addr: 0,
    width: 0,
    height: 0,
    pitch: 0,
    bpp: 0,
    bgr: true,
};
static mut CUR_R: u16 = 0;
static mut CUR_C: u16 = 0;
static mut VGA_MODE: bool = false;

/// BIOS path: 80×25 at `0xB8000`. Firmware already drew the flame.
pub fn offer_vga() {
    unsafe {
        core::ptr::addr_of_mut!(VGA_MODE).write(true);
        core::ptr::addr_of_mut!(FB).write(Fb {
            addr: 0,
            width: COLS as u64,
            height: ROWS as u64,
            pitch: 160,
            bpp: 0,
            bgr: true,
        });
    }
}

/// EFI / Limine GOP. `bgr` is true for OVMF/Bgr (u32 `0x00RRGGBB` LE).
#[allow(dead_code)]
pub fn offer_gop(addr: u64, width: u64, height: u64, pitch: u64, bpp: u16, bgr: bool) {
    if addr == 0 || bpp < 32 || width == 0 || height == 0 {
        return;
    }
    unsafe {
        core::ptr::addr_of_mut!(VGA_MODE).write(false);
        core::ptr::addr_of_mut!(FB).write(Fb {
            addr,
            width,
            height,
            pitch,
            bpp,
            bgr,
        });
    }
}

/// After CR3: map GOP UC if we have one, then paint Grove.
pub fn map_and_show() {
    let fb = unsafe { core::ptr::addr_of!(FB).read() };
    if fb.addr != 0 && fb.bpp >= 32 {
        let len = fb.height.saturating_mul(fb.pitch);
        crate::mm::map_uc(fb.addr, len);
    }
    show();
}

/// Paint without mapping (Limine still stands on firmware tables).
pub fn show() {
    if unsafe { core::ptr::addr_of!(VGA_MODE).read() } {
        show_vga();
    } else if unsafe { core::ptr::addr_of!(FB).read().addr } != 0 {
        show_gop();
    } else {
        return;
    }
    let msg = b"kindling\n";
    put_bytes(msg.as_ptr(), msg.len() as u64);
}

/// GOP physical addr, pitch, width, height. None if VGA-only or unset.
pub fn fb_info() -> Option<(u64, u64, u64, u64)> {
    let fb = unsafe { core::ptr::addr_of!(FB).read() };
    if fb.addr == 0 || fb.bpp < 32 {
        None
    } else {
        Some((fb.addr, fb.pitch, fb.width, fb.height))
    }
}

fn show_vga() {
    // Flame already at rows 0–6. Clearing from row 8, title at 14, log at 16.
    for r in 8..ROWS {
        for c in 0..COLS {
            vga_cell(r, c, b' ', 0x00);
        }
    }
    put_vga_art(8, GROVE, ATTR_VIOLET);
    put_vga_row(14, 0, TITLE.as_bytes(), ATTR_PARCHMENT);
    unsafe {
        core::ptr::addr_of_mut!(CUR_R).write(16);
        core::ptr::addr_of_mut!(CUR_C).write(0);
    }
}

fn show_gop() {
    // Glyphs only — a full UC fill of 1080p looks hung on iron. blit_char
    // already lays Night behind each cell.
    let mut y = 8u64;
    y = put_gop_art(GOP_X, y, FLAME, LILAC);
    y += 16;
    y = put_gop_art(GOP_X, y, GROVE, VIOLET);
    y += 16;
    put_gop_row(GOP_X, y, TITLE.as_bytes(), PARCHMENT);
    let row = ((y + 32) / 16) as u16;
    unsafe {
        core::ptr::addr_of_mut!(CUR_R).write(row.max(16));
        core::ptr::addr_of_mut!(CUR_C).write(0);
    }
    draw_cursor(unsafe { core::ptr::addr_of!(CUR_R).read() }, 0);
}

fn put_vga_art(mut row: u16, art: &str, attr: u8) {
    let mut col = 0u16;
    for b in art.bytes() {
        if b == b'\n' {
            row = row.saturating_add(1);
            col = 0;
            continue;
        }
        if row < ROWS && col < COLS {
            vga_cell(row, col, b, attr);
        }
        col = col.saturating_add(1);
    }
}

fn put_vga_row(row: u16, col: u16, s: &[u8], attr: u8) {
    for (i, &b) in s.iter().enumerate() {
        let c = col.saturating_add(i as u16);
        if c >= COLS || row >= ROWS {
            break;
        }
        vga_cell(row, c, b, attr);
    }
}

fn vga_cell(row: u16, col: u16, ch: u8, attr: u8) {
    let off = (row as u64 * 80 + col as u64) * 2;
    let p = (VGA + off) as *mut u8;
    unsafe {
        p.write_volatile(ch);
        p.add(1).write_volatile(attr);
    }
}

fn put_gop_art(x: u64, mut y: u64, art: &str, fg: u32) -> u64 {
    let mut col = 0u64;
    let start = y;
    for b in art.bytes() {
        if b == b'\n' {
            y += 16;
            col = 0;
            continue;
        }
        blit_char(x + col * 8, y, b, fg);
        col += 1;
    }
    if y == start {
        y += 16;
    }
    y
}

fn put_gop_row(x: u64, y: u64, s: &[u8], fg: u32) {
    for (col, &b) in s.iter().enumerate() {
        blit_char(x + col as u64 * 8, y, b, fg);
    }
}

fn blit_char(x: u64, y: u64, ch: u8, fg: u32) {
    let fb = unsafe { core::ptr::addr_of!(FB).read() };
    if fb.addr == 0 || (ch as usize) >= 128 {
        return;
    }
    let pix = pack(fg, fb.bgr);
    let night = pack(NIGHT, fb.bgr);
    let base = (ch as usize) * 16;
    if base + 16 > FONT.len() {
        return;
    }
    let mut row = 0u64;
    while row < 16 {
        let bits = FONT[base + row as usize];
        let mut col = 0u64;
        while col < 8 {
            let on = bits & (0x80 >> col) != 0;
            poke(fb, x + col, y + row, if on { pix } else { night });
            col += 1;
        }
        row += 1;
    }
}

fn pack(rgb: u32, bgr: bool) -> u32 {
    if bgr {
        rgb
    } else {
        let r = (rgb >> 16) & 0xFF;
        let g = (rgb >> 8) & 0xFF;
        let b = rgb & 0xFF;
        (b << 16) | (g << 8) | r
    }
}

fn poke(fb: Fb, x: u64, y: u64, pix: u32) {
    if x >= fb.width || y >= fb.height {
        return;
    }
    let off = y * fb.pitch + x * 4;
    unsafe {
        (fb.addr as *mut u8)
            .add(off as usize)
            .cast::<u32>()
            .write_volatile(pix);
    }
}

/// House `write` on the glass. Serial is the transcript; this is the picture.
pub fn put_bytes(buf: *const u8, len: u64) {
    if buf.is_null() || len == 0 {
        return;
    }
    let vga = unsafe { core::ptr::addr_of!(VGA_MODE).read() };
    let gop = unsafe { core::ptr::addr_of!(FB).read().addr } != 0;
    if !vga && !gop {
        return;
    }
    let mut i = 0u64;
    while i < len {
        let b = unsafe { buf.add(i as usize).read_volatile() };
        putc(b, vga);
        i += 1;
    }
}

fn gop_x(c: u16) -> u64 {
    GOP_X + c as u64 * 8
}

fn gop_y(r: u16) -> u64 {
    r as u64 * 16
}

fn draw_cursor(r: u16, c: u16) {
    let fb = unsafe { core::ptr::addr_of!(FB).read() };
    if fb.addr == 0 {
        return;
    }
    let pix = pack(PARCHMENT, fb.bgr);
    let x = gop_x(c);
    let y = gop_y(r) + 14;
    let mut col = 0u64;
    while col < 8 {
        poke(fb, x + col, y, pix);
        poke(fb, x + col, y + 1, pix);
        col += 1;
    }
}

fn erase_cursor(r: u16, c: u16) {
    blit_char(gop_x(c), gop_y(r), b' ', LILAC);
}

fn putc(b: u8, vga: bool) {
    let mut r = unsafe { core::ptr::addr_of!(CUR_R).read() };
    let mut c = unsafe { core::ptr::addr_of!(CUR_C).read() };
    let max_r = if vga {
        ROWS
    } else {
        let h = unsafe { core::ptr::addr_of!(FB).read().height };
        (h / 16) as u16
    };
    let max_c = if vga {
        COLS
    } else {
        let w = unsafe { core::ptr::addr_of!(FB).read().width };
        ((w.saturating_sub(GOP_X)) / 8).min(80) as u16
    };
    if !vga {
        erase_cursor(r, c);
    }
    if b == b'\n' {
        r = r.saturating_add(1);
        c = 0;
    } else if b == b'\r' {
        c = 0;
    } else if b == 8 {
        if c > 0 {
            c -= 1;
            if vga {
                vga_cell(r, c, b' ', ATTR_LILAC);
            } else {
                blit_char(gop_x(c), gop_y(r), b' ', LILAC);
            }
        }
    } else if (0x20..0x7F).contains(&b) {
        if vga {
            vga_cell(r, c, b, ATTR_LILAC);
        } else {
            blit_char(gop_x(c), gop_y(r), b, LILAC);
        }
        c = c.saturating_add(1);
        if c >= max_c {
            c = 0;
            r = r.saturating_add(1);
        }
    }
    if r >= max_r {
        r = 16;
        if r >= max_r {
            r = 0;
        }
    }
    unsafe {
        core::ptr::addr_of_mut!(CUR_R).write(r);
        core::ptr::addr_of_mut!(CUR_C).write(c);
    }
    if !vga {
        draw_cursor(r, c);
    }
}
