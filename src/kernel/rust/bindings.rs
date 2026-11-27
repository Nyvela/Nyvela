// C declarations used by the Rust kernel. Keep signatures in sync with the
// Nyvela C headers and bridge functions.
use core::ffi::c_char;

unsafe extern "C" {
    // include/nyvela/drivers/video/console/console.h
    pub(super) fn kwrite(buf: *const c_char, len: u64, color: u8);

    // src/kernel/rust_bridge.c
    pub(super) fn nyvela_rust_panic() -> !;
}
