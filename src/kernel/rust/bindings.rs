// C declarations used by the Rust kernel. Keep signatures in sync with the
// Lumen C headers and bridge functions.
use core::ffi::c_char;

unsafe extern "C" {
    // include/lumen/drivers/video/console/console.h
    pub(super) fn kwrite(buf: *const c_char, len: u64, color: u8);

    // src/kernel/rust_bridge.c
    pub(super) fn lumen_rust_panic() -> !;

    // include/lumen/mm/{pmm,vmm}.h
    pub(super) fn kpalloc() -> *mut u8;
    pub(super) fn kpalloc_contiguous(pages: u64) -> *mut u8;
    pub(super) fn kpfree_contiguous(page: *mut u8, pages: u64);
    pub(super) fn kpfree(page: *mut u8);
    pub(super) fn kget_pte_addr_at(cr3: u64, virt: u64) -> *const u64;
    pub(super) fn kvmmap_at(cr3: u64, virt: u64, phys: u64, flags: u64) -> bool;
    pub(super) fn kvmunmap_and_free_at(cr3: u64, virt: u64) -> bool;
    pub(super) fn kvmunmap_at(cr3: u64, virt: u64) -> bool;
    pub(super) static kernel_cr3: u64;

    pub(super) fn kmalloc(size: u64) -> *mut u8;
    pub(super) fn kfree(pointer: *mut u8);
    pub(super) fn vfs_read(path: *const c_char, output: *mut u8, len: u64, offset: u64) -> i64;
}
