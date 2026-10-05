#![no_std]
#![no_main]

//! Our BOOTX64.EFI — other people's firmware, our kernel.

mod ata;
mod cairn;
mod cpu;
mod gdt;
mod glass;
mod house;
mod idt;
mod kbd;
mod mm;
mod start;
mod timer;

use core::fmt::Write;
use mm::Hint;
use start::{serial_print, start, Serial};
use uefi::boot::{AllocateType, MemoryType, SearchType};
use uefi::cstr16;
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::device_path::{DevicePath, LoadedImageDevicePath};
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileMode, RegularFile};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::ProtocolPointer;

#[used]
static CLOTHES: &[u8] = b"not their OS; our clothes\0";

fn conout(s: &str) {
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.write_str(s);
    });
}

struct CairnBuf {
    src: u64,
    len: u64,
}

unsafe fn get_proto<P: ProtocolPointer + ?Sized>(
    handle: uefi::Handle,
) -> Option<uefi::boot::ScopedProtocol<P>> {
    let params = uefi::boot::OpenProtocolParams {
        handle,
        agent: uefi::boot::image_handle(),
        controller: None,
    };
    unsafe {
        uefi::boot::open_protocol::<P>(params, uefi::boot::OpenProtocolAttributes::GetProtocol).ok()
    }
}

/// LocateDevicePath wants a path that *contains* the HD node. A USB
/// parent path is too short; the loaded-image path includes the file.
fn fs_from_path(path: &DevicePath) -> Option<uefi::boot::ScopedProtocol<SimpleFileSystem>> {
    let mut remaining = path;
    let fs_handle = uefi::boot::locate_device_path::<SimpleFileSystem>(&mut remaining).ok()?;
    unsafe { get_proto::<SimpleFileSystem>(fs_handle) }
}

/// OVMF often has SimpleFileSystem on the loaded-image device. Insyde USB
/// puts it on a child handle — walk the device path (GetProtocol, never
/// exclusive: Insyde hung on exclusive GOP).
fn volume_fs(dev: uefi::Handle) -> Option<uefi::boot::ScopedProtocol<SimpleFileSystem>> {
    if let Some(fs) = unsafe { get_proto::<SimpleFileSystem>(dev) } {
        return Some(fs);
    }
    let path = unsafe { get_proto::<DevicePath>(dev) }?;
    fs_from_path(path.get()?)
}

/// Full USB+HD+file path on the image handle. Prefix of that is the volume.
fn image_volume_fs() -> Option<uefi::boot::ScopedProtocol<SimpleFileSystem>> {
    let path = unsafe { get_proto::<LoadedImageDevicePath>(uefi::boot::image_handle()) }?;
    fs_from_path(path.get()?)
}

fn open_cairn_file(root: &mut impl File) -> Option<RegularFile> {
    for path in [cstr16!("EFI\\BOOT\\CAIRN"), cstr16!("\\EFI\\BOOT\\CAIRN")] {
        if let Ok(fh) = root.open(path, FileMode::Read, FileAttribute::empty()) {
            if let Some(f) = fh.into_regular_file() {
                return Some(f);
            }
        }
    }
    let efi = root
        .open(cstr16!("EFI"), FileMode::Read, FileAttribute::empty())
        .ok()?;
    let mut efi = efi.into_directory()?;
    let boot = efi
        .open(cstr16!("BOOT"), FileMode::Read, FileAttribute::empty())
        .ok()?;
    let mut boot = boot.into_directory()?;
    boot.open(cstr16!("CAIRN"), FileMode::Read, FileAttribute::empty())
        .ok()?
        .into_regular_file()
}

/// `(addr, cap_bytes)`. Insyde often refuses 512 KiB at exactly 1MB;
/// the live cairn is one sector, so shrink the request before giving up.
fn alloc_cairn(want_pages: usize) -> Option<(u64, u64)> {
    let tries = [want_pages, 16, 4, 1];
    for &n in &tries {
        if n == 0 || n > want_pages {
            continue;
        }
        if uefi::boot::allocate_pages(
            AllocateType::Address(crate::cairn::CAIRN_RAM),
            MemoryType::LOADER_DATA,
            n,
        )
        .is_ok()
        {
            return Some((crate::cairn::CAIRN_RAM, n as u64 * 4096));
        }
    }
    for &n in &tries {
        if n == 0 || n > want_pages {
            continue;
        }
        if let Ok(p) =
            uefi::boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, n)
        {
            return Some((p.as_ptr() as u64, n as u64 * 4096));
        }
    }
    None
}

fn slot_usable<M: MemoryMap>(map: &M, len: u64) -> bool {
    let start = crate::cairn::CAIRN_RAM;
    let end = start.saturating_add(len.max(1));
    for d in map.entries() {
        let a = d.phys_start;
        let b = a.saturating_add(d.page_count.saturating_mul(4096));
        if a <= start && end <= b {
            return matches!(
                d.ty,
                MemoryType::CONVENTIONAL
                    | MemoryType::LOADER_DATA
                    | MemoryType::LOADER_CODE
                    | MemoryType::BOOT_SERVICES_CODE
                    | MemoryType::BOOT_SERVICES_DATA
            );
        }
    }
    false
}

fn overlaps_image(len: u64, img_base: u64, img_end: u64) -> bool {
    if img_base == 0 || img_end <= img_base {
        return false;
    }
    let a = crate::cairn::CAIRN_RAM;
    let b = a.saturating_add(len.max(1));
    a < img_end && img_base < b
}

fn load_from_fs(mut fs: uefi::boot::ScopedProtocol<SimpleFileSystem>) -> Option<CairnBuf> {
    let mut root = fs.open_volume().ok()?;
    let mut file = open_cairn_file(&mut root)?;
    let max = 1024u64 * 512;
    let pages = (max / 4096) as usize;
    let (dest, cap) = alloc_cairn(pages)?;
    let dest_ptr = dest as *mut u8;
    let mut chunk = [0u8; 4096];
    let mut off = 0u64;
    loop {
        let Ok(n) = file.read(&mut chunk) else {
            return None;
        };
        if n == 0 {
            break;
        }
        let n = n as u64;
        if off.saturating_add(n) > max || off.saturating_add(n) > cap {
            return None;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(chunk.as_ptr(), dest_ptr.add(off as usize), n as usize);
        }
        off += n;
    }
    if off == 0 {
        return None;
    }
    let padded = (off + 511) & !511;
    if padded > cap {
        return None;
    }
    unsafe {
        core::ptr::write_bytes(dest_ptr.add(off as usize), 0, (padded - off) as usize);
    }
    conout("cairn\r\n");
    Some(CairnBuf { src: dest, len: padded })
}

/// Copy `\EFI\BOOT\CAIRN` while boot services still live. `offer` plants
/// KMAP *after* ExitBootServices (0x8400 is firmware RAM until then).
fn load_cairn(dev: Option<uefi::Handle>) -> Option<CairnBuf> {
    if let Some(d) = dev {
        if let Some(fs) = volume_fs(d) {
            if let Some(buf) = load_from_fs(fs) {
                return Some(buf);
            }
        }
    }
    if let Some(fs) = image_volume_fs() {
        if let Some(buf) = load_from_fs(fs) {
            return Some(buf);
        }
    }
    let handles =
        uefi::boot::locate_handle_buffer(SearchType::from_proto::<SimpleFileSystem>()).ok()?;
    for &h in handles.iter() {
        if let Some(fs) = unsafe { get_proto::<SimpleFileSystem>(h) } {
            if let Some(buf) = load_from_fs(fs) {
                return Some(buf);
            }
        }
    }
    None
}

fn plant_cairn<M: MemoryMap>(map: &M, buf: CairnBuf, img_base: u64, img_end: u64) {
    if buf.src != crate::cairn::CAIRN_RAM {
        if overlaps_image(buf.len, img_base, img_end) {
            return;
        }
        if !slot_usable(map, buf.len) {
            return;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(
                buf.src as *const u8,
                crate::cairn::CAIRN_RAM as *mut u8,
                buf.len as usize,
            );
        }
    }
    crate::cairn::offer(buf.len);
}

fn conout_art(art: &str) {
    uefi::system::with_stdout(|stdout| {
        for line in art.split('\n') {
            if line.is_empty() {
                continue;
            }
            let _ = stdout.write_str(line);
            let _ = stdout.write_str("\r\n");
        }
    });
}

#[entry]
fn efi_main() -> Status {
    let _ = core::hint::black_box(CLOTHES);
    uefi::helpers::init().ok();
    conout("cerne-efi\r\n");
    // Grove on ConOut *before* GOP: Insyde iron hangs inside GOP open, and
    // the panel is the only transcript (no COM1). Glyph first, then the rest.
    conout_art(crate::glass::GROVE);
    conout_art(crate::glass::TITLE);
    let mut kernel_end = 0x40_0000u64;
    let mut img_base = 0u64;
    let mut device = None;
    // GetProtocol, not exclusive — our own image handle. Safe on Insyde.
    let lip = uefi::boot::OpenProtocolParams {
        handle: uefi::boot::image_handle(),
        agent: uefi::boot::image_handle(),
        controller: None,
    };
    if let Ok(img) = unsafe {
        uefi::boot::open_protocol::<LoadedImage>(
            lip,
            uefi::boot::OpenProtocolAttributes::GetProtocol,
        )
    } {
        let (base, size) = img.info();
        img_base = base as u64;
        kernel_end = img_base.saturating_add(size);
        device = img.device();
    }
    conout("image\r\n");
    let cairn_buf = load_cairn(device);
    let mut fb = (core::ptr::null_mut(), 0u64, 0u64, 0u64, 0u16, true);
    // GetProtocol, not exclusive: the Insyde test iron hangs inside an
    // exclusive GOP open (QEMU's OVMF says yes). We only read the mode and
    // the framebuffer address, so the lightest open is also the honest one.
    // Unsafe: application use, image handle as agent, dropped before
    // ExitBootServices — the handle provably outlives the scope.
    if let Ok(handle) = uefi::boot::get_handle_for_protocol::<GraphicsOutput>() {
        let params = uefi::boot::OpenProtocolParams {
            handle,
            agent: uefi::boot::image_handle(),
            controller: None,
        };
        if let Ok(mut gop) = unsafe {
            uefi::boot::open_protocol::<GraphicsOutput>(
                params,
                uefi::boot::OpenProtocolAttributes::GetProtocol,
            )
        } {
            let mode = gop.current_mode_info();
            let mut region = gop.frame_buffer();
            let (bpp, bgr) = match mode.pixel_format() {
                PixelFormat::Bgr => (32, true),
                PixelFormat::Rgb => (32, false),
                _ => (0, true),
            };
            let (w, h) = mode.resolution();
            fb = (
                region.as_mut_ptr(),
                w as u64,
                h as u64,
                mode.stride() as u64 * 4,
                bpp,
                bgr,
            );
        }
    }
    if !fb.0.is_null() && fb.4 >= 32 {
        conout("gop\r\n");
        crate::glass::offer_gop(fb.0 as u64, fb.1, fb.2, fb.3, fb.4, fb.5);
        crate::glass::show();
    } else {
        conout("gop skip\r\n");
    }
    conout("exit\r\n");
    let map = unsafe { uefi::boot::exit_boot_services(None) };
    if let Some(buf) = cairn_buf {
        plant_cairn(&map, buf, img_base, kernel_end);
    }
    let mut ram_end = 0u64;
    for d in map.entries() {
        if d.ty == MemoryType::CONVENTIONAL {
            let end = d
                .phys_start
                .saturating_add(d.page_count.saturating_mul(4096));
            ram_end = ram_end.max(end);
        }
    }
    let ram_end = ram_end
        .max(kernel_end.saturating_add(0x20_0000))
        .min(1 << 30);
    start(Some(Hint {
        kernel_end,
        ram_end,
        trust_map: true,
    }))
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    start::serial_init();
    serial_print("the spark went out: ");
    let _ = writeln!(Serial, "{info}");
    start::hcf();
}
