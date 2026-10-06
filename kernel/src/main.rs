#![no_std]
#![no_main]

mod ata;
mod cairn;
mod cpu;
mod gdt;
mod glass;
mod house;
mod idt;
mod kbd;
mod mm;
mod pci;
mod usb;
mod start;
mod timer;

use core::fmt::Write;

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};

use start::{Serial, serial_print, start};

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    let _ = &BASE_REVISION;
    if !BASE_REVISION.is_supported() {
        start::serial_init();
        serial_print("limine revision not supported\n");
        start::hcf();
    }
    if let Some(resp) = FRAMEBUFFER_REQUEST.get_response()
        && let Some(fb) = resp.framebuffers().next()
    {
        crate::glass::offer_gop(
            fb.addr() as u64,
            fb.width(),
            fb.height(),
            fb.pitch(),
            fb.bpp(),
            true,
        );
    }
    start(None)
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    start::serial_init();
    serial_print("the spark went out: ");
    let _ = writeln!(Serial, "{info}");
    start::hcf();
}
