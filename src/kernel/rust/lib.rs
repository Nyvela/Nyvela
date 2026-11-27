#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

mod acpi;
mod bindings;
mod console;
mod cpuinfo;
mod exec;
mod smp;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    // SAFETY: The fatal C handler disables interrupts and never returns.
    unsafe { bindings::nyvela_rust_panic() }
}

/// # Safety
///
/// Call during early kernel boot after console setup and before interrupt
/// or scheduler startup. The caller must exclude other console access.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nyvela_rust_init() {
    // SAFETY: The C caller guarantees exclusive early-boot console access.
    let mut console = unsafe { console::BootConsole::new() };
    start(&mut console);
}

fn start(console: &mut console::BootConsole) {
    console.write("Nyvela Rust kernel ready.\n");

    let info = cpuinfo::collect();
    cpuinfo::report(console, &info);
}
