//! Our BOOTX64.EFI — other people's firmware, our kernel.
#![no_std]
#![no_main]

mod start;

use core::fmt::Write;
use start::{paint_mark, serial_print, start, Serial};
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

#[entry]
fn efi_main() -> Status {
    uefi::helpers::init().ok();
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.write_str("cerne-efi\r\n");
    });
    let mut fb = (core::ptr::null_mut(), 0u64, 0u64, 0u64, 0u16);
    if let Ok(handle) = uefi::boot::get_handle_for_protocol::<GraphicsOutput>() {
        if let Ok(mut gop) = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(handle) {
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
    let _map = unsafe { uefi::boot::exit_boot_services(None) };
    if !fb.0.is_null() {
        paint_mark(fb.0, fb.1, fb.2, fb.3, fb.4);
    }
    start()
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    start::serial_init();
    serial_print("panic: ");
    let _ = writeln!(Serial, "{info}");
    start::hcf();
}
