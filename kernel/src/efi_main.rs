#![no_std]
#![no_main]

//! Our BOOTX64.EFI — other people's firmware, our kernel.

mod ata;
mod cairn;
mod cpu;
mod gdt;
mod house;
mod idt;
mod mm;
mod start;
mod timer;

use core::fmt::Write;
use mm::Hint;
use start::{Serial, paint_mark, serial_print, start};
use uefi::boot::MemoryType;
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::loaded_image::LoadedImage;

#[used]
static CLOTHES: &[u8] = b"not their OS; our clothes\0";

#[entry]
fn efi_main() -> Status {
    let _ = core::hint::black_box(CLOTHES);
    uefi::helpers::init().ok();
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.write_str("cerne-efi\r\n");
    });
    let mut fb = (core::ptr::null_mut(), 0u64, 0u64, 0u64, 0u16);
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
            let bpp = match mode.pixel_format() {
                PixelFormat::Rgb | PixelFormat::Bgr => 32,
                _ => 0,
            };
            let (w, h) = mode.resolution();
            fb = (
                region.as_mut_ptr(),
                w as u64,
                h as u64,
                mode.stride() as u64 * 4,
                bpp,
            );
        }
    }
    let mut kernel_end = 0x40_0000u64;
    // GetProtocol here too — same firmware, same rule.
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
    }
    let map = unsafe { uefi::boot::exit_boot_services(None) };
    if !fb.0.is_null() {
        paint_mark(fb.0, fb.1, fb.2, fb.3, fb.4);
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
