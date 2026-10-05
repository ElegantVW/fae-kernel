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
use uefi::boot::{AllocateType, MemoryType};
use uefi::cstr16;
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;

#[used]
static CLOTHES: &[u8] = b"not their OS; our clothes\0";

fn conout(s: &str) {
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.write_str(s);
    });
}

/// Copy `\EFI\BOOT\CAIRN` to `0x100000` while boot services still live.
/// Returns the sector-padded length so `offer` can plant KMAP *after*
/// ExitBootServices (0x8400 is firmware RAM until then).
fn load_cairn(dev: uefi::Handle) -> Option<u64> {
    let params = uefi::boot::OpenProtocolParams {
        handle: dev,
        agent: uefi::boot::image_handle(),
        controller: None,
    };
    let Ok(mut fs) = (unsafe {
        uefi::boot::open_protocol::<SimpleFileSystem>(
            params,
            uefi::boot::OpenProtocolAttributes::GetProtocol,
        )
    }) else {
        return None;
    };
    let Ok(mut root) = fs.open_volume() else {
        return None;
    };
    let Ok(fh) = root.open(
        cstr16!("EFI\\BOOT\\CAIRN"),
        FileMode::Read,
        FileAttribute::empty(),
    ) else {
        return None;
    };
    let Some(mut file) = fh.into_regular_file() else {
        return None;
    };
    let max = 1024u64 * 512;
    let pages = (max / 4096) as usize;
    if uefi::boot::allocate_pages(
        AllocateType::Address(crate::cairn::CAIRN_RAM),
        MemoryType::LOADER_DATA,
        pages,
    )
    .is_err()
    {
        return None;
    }
    let dest = crate::cairn::CAIRN_RAM as *mut u8;
    let mut chunk = [0u8; 4096];
    let mut off = 0u64;
    loop {
        let Ok(n) = file.read(&mut chunk) else {
            return None;
        };
        if n == 0 {
            break;
        }
        if off.saturating_add(n as u64) > max {
            return None;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(chunk.as_ptr(), dest.add(off as usize), n);
        }
        off += n as u64;
    }
    if off == 0 {
        return None;
    }
    let padded = (off + 511) & !511;
    unsafe {
        core::ptr::write_bytes(dest.add(off as usize), 0, (padded - off) as usize);
    }
    conout("cairn\r\n");
    Some(padded)
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
        kernel_end = (base as u64).saturating_add(size);
        device = img.device();
    }
    conout("image\r\n");
    let mut cairn_len = None;
    if let Some(dev) = device {
        cairn_len = load_cairn(dev);
    }
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
    if let Some(len) = cairn_len {
        crate::cairn::offer(len);
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
