use core::ptr;

use crate::bindings;

pub const PAGE: u64 = 4096;
pub const PRESENT: u64 = 1;
pub const WRITE: u64 = 2;
pub const USER: u64 = 4;
pub const NX: u64 = 1 << 63;
const OWNED: u64 = 1 << 9;
pub const NO_ACCESS: u64 = 1 << 10;
const PHYS: u64 = 0x000f_ffff_ffff_f000;

pub const IMAGE_LOW: core::ops::Range<u64> = 0x400000..0x800000;
pub const IMAGE_HIGH: core::ops::Range<u64> = 0x40000000..0xc0000000;
pub const MAP_FIRST: u64 = 0x20_0000_0000;
pub const MAP_END: u64 = 0x40_0000_0000;
pub const STACK_TOP: u64 = 0x7fff_ffff_f000;
pub const STACK_SIZE: u64 = 1024 * 1024;

pub fn page_floor(address: u64) -> u64 {
    address & !(PAGE - 1)
}

pub fn page_ceil(address: u64) -> Option<u64> {
    address.checked_add(PAGE - 1).map(page_floor)
}

pub fn image_range(first: u64, end: u64) -> bool {
    [IMAGE_LOW, IMAGE_HIGH]
        .iter()
        .any(|range| first >= range.start && first < end && end <= range.end)
}

pub fn entry(cr3: u64, address: u64) -> u64 {
    let pte = unsafe { bindings::kget_pte_addr_at(cr3, address) };
    unsafe { pte.as_ref() }.copied().unwrap_or_default()
}

fn allocate() -> Option<u64> {
    let page = unsafe { bindings::kpalloc() };
    if page.is_null() {
        return None;
    }
    unsafe { page.write_bytes(0, PAGE as usize) };
    Some(page.addr() as u64)
}

fn free(phys: u64) {
    unsafe { bindings::kpfree(phys as *mut u8) };
}

pub fn map_new(cr3: u64, address: u64, flags: u64) -> bool {
    let Some(phys) = allocate() else { return false };
    if map(cr3, address, phys, flags) {
        true
    } else {
        free(phys);
        false
    }
}

fn map(cr3: u64, address: u64, phys: u64, flags: u64) -> bool {
    let user = if flags & NO_ACCESS == 0 { USER } else { 0 };
    unsafe { bindings::kvmmap_at(cr3, address, phys, flags | PRESENT | OWNED | user) }
}

pub fn owned(cr3: u64, address: u64) -> bool {
    entry(cr3, address) & (PRESENT | OWNED) == PRESENT | OWNED
}

pub fn protect(cr3: u64, address: u64, flags: u64) -> bool {
    let old = entry(cr3, address);
    old & (PRESENT | OWNED) == PRESENT | OWNED && map(cr3, address, old & PHYS, flags)
}

pub fn unmap(cr3: u64, address: u64) {
    if owned(cr3, address) {
        unsafe { bindings::kvmunmap_and_free_at(cr3, address) };
    }
}

fn phys(cr3: u64, address: u64, required: u64) -> Option<*mut u8> {
    if !(IMAGE_LOW.contains(&address)
        || IMAGE_HIGH.contains(&address)
        || (MAP_FIRST..MAP_END).contains(&address)
        || (STACK_TOP - STACK_SIZE..STACK_TOP).contains(&address))
    {
        return None;
    }
    let pte = entry(cr3, address);
    let required = required | PRESENT | USER;
    if pte & required != required {
        return None;
    }
    Some(((pte & PHYS) | (address & (PAGE - 1))) as *mut u8)
}

pub fn valid(cr3: u64, address: u64, length: usize, flags: u64) -> bool {
    let Some(end) = address.checked_add(length as u64) else {
        return false;
    };
    let mut position = address;
    while position < end {
        if phys(cr3, position, flags).is_none() {
            return false;
        }
        position = page_floor(position) + PAGE;
    }
    true
}

pub fn copy_in(cr3: u64, address: u64, out: &mut [u8]) -> bool {
    transfer(cr3, address, out.len(), 0, |page, offset, count| unsafe {
        ptr::copy_nonoverlapping(page, out.as_mut_ptr().add(offset), count);
    })
}

pub fn copy_out(cr3: u64, address: u64, data: &[u8]) -> bool {
    copy_to(cr3, address, data, WRITE)
}

pub fn load_bytes(cr3: u64, address: u64, data: &[u8]) -> bool {
    copy_to(cr3, address, data, 0)
}

fn copy_to(cr3: u64, address: u64, data: &[u8], flags: u64) -> bool {
    transfer(
        cr3,
        address,
        data.len(),
        flags,
        |page, offset, count| unsafe {
            ptr::copy_nonoverlapping(data.as_ptr().add(offset), page, count);
        },
    )
}

fn transfer(
    cr3: u64,
    address: u64,
    length: usize,
    flags: u64,
    mut chunk: impl FnMut(*mut u8, usize, usize),
) -> bool {
    if !valid(cr3, address, length, flags) {
        return false;
    }
    let mut offset = 0;
    while offset < length {
        let position = address + offset as u64;
        let count = ((PAGE - (position & (PAGE - 1))) as usize).min(length - offset);
        let Some(page) = phys(cr3, position, flags) else {
            return false;
        };
        chunk(page, offset, count);
        offset += count;
    }
    true
}

type Table = [u64; 512];

fn table(phys: u64) -> *mut Table {
    (phys & PHYS) as *mut Table
}

pub struct Space(pub u64);

impl Space {
    pub fn new() -> Option<Self> {
        // Allocate all private copies before installing any shared pointers.
        let mut pages = [0; 5];
        for i in 0..pages.len() {
            let Some(page) = allocate() else {
                for page in &pages[..i] {
                    free(*page);
                }
                return None;
            };
            pages[i] = page;
        }

        unsafe {
            let kernel = table(bindings::kernel_cr3);
            let kernel_pdpt = table((*kernel)[0]);
            let kernel_pd = table((*kernel_pdpt)[0]);

            let [pml4, pdpt, pd, low_pt, high_pt] = pages.map(table);
            pml4.copy_from_nonoverlapping(kernel, 1);
            pdpt.copy_from_nonoverlapping(kernel_pdpt, 1);
            pd.copy_from_nonoverlapping(kernel_pd, 1);
            low_pt.copy_from_nonoverlapping(table((*kernel_pd)[2]), 1);
            high_pt.copy_from_nonoverlapping(table((*kernel_pd)[3]), 1);
            (*pml4)[0] = pages[1] | PRESENT | WRITE;
            (*pdpt)[0] = pages[2] | PRESENT | WRITE;
            (*pd)[2] = pages[3] | PRESENT | WRITE;
            (*pd)[3] = pages[4] | PRESENT | WRITE;
        }
        Some(Self(pages[0]))
    }
}

impl Drop for Space {
    fn drop(&mut self) {
        unsafe { release(self.0, bindings::kernel_cr3, 4) };
    }
}

pub fn self_test() -> Option<()> {
    let first = Space::new()?;
    let second = Space::new()?;
    let address = MAP_FIRST;
    let mut result = [0; 4];
    (map_new(first.0, address, WRITE | NX)
        && map_new(second.0, address, WRITE | NX)
        && copy_out(first.0, address, b"test")
        && copy_in(second.0, address, &mut result)
        && result == [0; 4]
        && protect(first.0, address, NO_ACCESS | NX)
        && !copy_in(first.0, address, &mut result)
        && protect(first.0, address, NX)
        && !copy_out(first.0, address, b"fail")
        && copy_in(first.0, address, &mut result)
        && result == *b"test")
        .then_some(())
}

/// Free private tables and user-owned leaves. Kernel identity entries,
/// including the entries replaced in the two private low tables, are shared.
unsafe fn release(phys: u64, kernel: u64, level: usize) {
    let private = table(phys);
    let shared = table(kernel);
    if private == shared {
        return;
    }
    unsafe {
        for i in 0..512 {
            let slot = (*private)[i];
            if slot & PRESENT == 0 {
                continue;
            }
            if level == 1 {
                if slot & OWNED != 0 {
                    free(slot & PHYS);
                }
            } else {
                let source = if shared.is_null() { 0 } else { (*shared)[i] };
                release(slot, source, level - 1);
            }
        }
    }
    free(phys & PHYS);
}
