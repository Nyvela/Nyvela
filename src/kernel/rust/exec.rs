use crate::bindings;

const PAGE_SIZE: u64 = 4096;
const STACK_PAGES: u64 = 8;
const USER_WRITABLE: u64 = 7;
const PRESENT_USER: u64 = 5;

#[repr(C)]
pub struct Image {
    bytes: *const u8,
    length: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn nyvela_fat_image() -> Image {
    let bytes = include_bytes!(env!("NYVELA_FAT_BIN"));
    Image {
        bytes: bytes.as_ptr(),
        length: bytes.len(),
    }
}

/// Adds pages below the initial exec stack page. The caller destroys the
/// child address space and any successfully mapped pages on failure.
///
/// # Safety
///
/// The address space must be owned by the caller and inactive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nyvela_map_exec_stack(cr3: u64, initial_page: u64) -> bool {
    for index in 1..STACK_PAGES {
        let virtual_page = initial_page - index * PAGE_SIZE;
        let pte = unsafe { bindings::kget_pte_addr_at(cr3, virtual_page) };
        let entry = unsafe { pte.as_ref() }.copied().unwrap_or(0);
        if entry & PRESENT_USER == PRESENT_USER {
            if entry & USER_WRITABLE != USER_WRITABLE {
                return false;
            }
            continue;
        }
        // The existing allocator returns a private, identity-mapped page.
        let page = unsafe { bindings::kpalloc() };
        if page.is_null() {
            return false;
        }
        unsafe { page.write_bytes(0, PAGE_SIZE as usize) };
        if !unsafe { bindings::kvmmap_at(cr3, virtual_page, page.addr() as u64, USER_WRITABLE) } {
            unsafe { bindings::kpfree(page) };
            return false;
        }
    }
    true
}
