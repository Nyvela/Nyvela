//! The boot volume. All operations run on the boot CPU with preemption
//! disabled. File data lives in pages and grows independently of directory nodes.
use core::{ffi::c_char, ptr, slice};

use crate::bindings;

const PAGE: usize = 4096;
const SLOTS: usize = PAGE / size_of::<*mut u8>() - 1;
const MAX_FILE: usize = 64 * 1024 * 1024;
const INVALID: i64 = -1;
const MISSING: i64 = -2;
const EXISTS: i64 = -3;
const NOT_DIR: i64 = -4;
const IS_DIR: i64 = -5;
const NO_SPACE: i64 = -6;
const BAD_PATH: i64 = -7;

#[repr(C)]
struct Pages {
    next: *mut Self,
    data: [*mut u8; SLOTS],
}

struct Node {
    name: [u8; 64],
    name_len: usize,
    directory: bool,
    size: usize,
    inode: u64,
    parent_inode: u64,
    references: usize,
    linked: bool,
    child: *mut Node,
    sibling: *mut Node,
    pages: *mut Pages,
}

static mut ROOT: *mut Node = ptr::null_mut();
static mut NEXT_INODE: u64 = 1;

fn allocate_page() -> *mut u8 {
    let page = unsafe { bindings::kpalloc() };
    if !page.is_null() {
        unsafe { page.write_bytes(0, PAGE) };
    }
    page
}

fn new_node(name: &[u8], directory: bool) -> *mut Node {
    let node = unsafe { bindings::kmalloc(size_of::<Node>() as u64) }.cast::<Node>();
    if !node.is_null() {
        let mut stored = [0; 64];
        stored[..name.len()].copy_from_slice(name);
        unsafe {
            node.write(Node {
                name: stored,
                name_len: name.len(),
                directory,
                size: 0,
                inode: NEXT_INODE,
                parent_inode: NEXT_INODE,
                references: 0,
                linked: true,
                child: ptr::null_mut(),
                sibling: ptr::null_mut(),
                pages: ptr::null_mut(),
            });
            NEXT_INODE += 1;
        }
    }
    node
}

unsafe fn path_bytes<'a>(path: *const c_char) -> Option<&'a [u8]> {
    if path.is_null() {
        return None;
    }
    let path = path.cast::<u8>();
    for length in 0..=256 {
        if unsafe { path.add(length).read() } == 0 {
            return Some(unsafe { slice::from_raw_parts(path, length) });
        }
    }
    None
}

fn child(parent: *mut Node, name: &[u8]) -> Result<*mut Node, i64> {
    unsafe {
        if parent.is_null() || !(*parent).directory {
            return Err(NOT_DIR);
        }
        let mut node = (*parent).child;
        while let Some(entry) = node.as_ref() {
            if entry.name[..entry.name_len] == *name {
                return Ok(node);
            }
            node = entry.sibling;
        }
    }
    Err(MISSING)
}

fn walk(
    mut node: *mut Node,
    components: impl Iterator<Item = impl AsRef<[u8]>>,
) -> Result<*mut Node, i64> {
    for component in components {
        let name = component.as_ref();
        if name.is_empty() || name.len() >= 64 {
            return Err(BAD_PATH);
        }
        node = child(node, name)?;
    }
    Ok(node)
}

fn lookup(path: &[u8]) -> Result<*mut Node, i64> {
    let rest = path.strip_prefix(b"/").ok_or(BAD_PATH)?;
    let root = unsafe { ROOT };
    if root.is_null() {
        return Err(INVALID);
    }
    if rest.is_empty() {
        return Ok(root);
    }
    walk(root, rest.split(|&byte| byte == b'/'))
}

fn parent(path: &[u8]) -> Result<(*mut Node, &[u8]), i64> {
    let split = path
        .iter()
        .rposition(|&byte| byte == b'/')
        .ok_or(BAD_PATH)?;
    let leaf = &path[split + 1..];
    if leaf.is_empty() || leaf.len() >= 64 {
        return Err(BAD_PATH);
    }
    let dir = lookup(if split == 0 { b"/" } else { &path[..split] })?;
    if !unsafe { (*dir).directory } {
        return Err(NOT_DIR);
    }
    Ok((dir, leaf))
}

#[unsafe(no_mangle)]
extern "C" fn ramfs_init() -> bool {
    unsafe {
        if ROOT.is_null() {
            ROOT = new_node(b"", true);
        }
        !ROOT.is_null()
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn ramfs_create(path: *const c_char, directory: bool) -> i32 {
    let Some(path) = (unsafe { path_bytes(path) }) else {
        return BAD_PATH as i32;
    };
    create(path, directory) as i32
}

pub fn create(path: &[u8], directory: bool) -> i64 {
    let (dir, name) = match parent(path) {
        Ok(value) => value,
        Err(error) => return error,
    };
    if child(dir, name).is_ok() {
        return EXISTS;
    }
    let node = new_node(name, directory);
    if node.is_null() {
        return NO_SPACE;
    }
    unsafe {
        (*node).parent_inode = (*dir).inode;
        (*node).sibling = (*dir).child;
        (*dir).child = node;
    }
    0
}

pub fn stat(path: &[u8]) -> Result<(bool, usize), i64> {
    let node = lookup(path)?;
    unsafe { Ok(((*node).directory, (*node).size)) }
}

pub fn stat_with_inode(path: &[u8]) -> Result<(bool, usize, u64), i64> {
    let node = unsafe { &*lookup(path)? };
    Ok((node.directory, node.size, node.inode))
}

pub struct Handle(*mut Node);

pub fn open(path: &[u8]) -> Result<Handle, i64> {
    let node = lookup(path)?;
    unsafe { (*node).references += 1 };
    Ok(Handle(node))
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let node = &mut *self.0;
            node.references -= 1;
            if node.references == 0 && !node.linked {
                destroy(self.0);
            }
        }
    }
}

impl Handle {
    pub fn stat(&self) -> (bool, usize, u64) {
        let node = unsafe { &*self.0 };
        (node.directory, node.size, node.inode)
    }

    pub fn read(&self, out: &mut [u8], offset: u64) -> i64 {
        if self.stat().0 {
            IS_DIR
        } else {
            read_node(self.0, out, offset)
        }
    }

    pub fn write(&self, data: &[u8], offset: u64) -> i64 {
        if self.stat().0 {
            IS_DIR
        } else {
            write_node(self.0, data, offset)
        }
    }

    pub fn truncate(&self, size: u64) -> i64 {
        if self.stat().0 {
            IS_DIR
        } else {
            truncate_node(self.0, size)
        }
    }

    pub fn directory_entry(&self, index: usize) -> Option<DirectoryEntry> {
        let directory = unsafe { &*self.0 };
        if !directory.directory {
            return None;
        }
        if index < 2 {
            let mut name = [0; 64];
            name[..=index].fill(b'.');
            return Some(DirectoryEntry {
                name,
                length: index + 1,
                inode: if index == 0 {
                    directory.inode
                } else {
                    directory.parent_inode
                },
                directory: true,
            });
        }
        let mut current = directory.child;
        for _ in 2..index {
            current = unsafe { current.as_ref() }?.sibling;
        }
        let current = unsafe { current.as_ref() }?;
        Some(DirectoryEntry {
            name: current.name,
            length: current.name_len,
            inode: current.inode,
            directory: current.directory,
        })
    }
}

pub struct DirectoryEntry {
    pub name: [u8; 64],
    pub length: usize,
    pub inode: u64,
    pub directory: bool,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn ramfs_size(path: *const c_char) -> i64 {
    let Some(path) = (unsafe { path_bytes(path) }) else {
        return BAD_PATH;
    };
    match stat(path) {
        Ok((true, _)) => IS_DIR,
        Ok((false, size)) => size as i64,
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn ramfs_list(path: *const c_char, out: *mut u8, len: usize) -> i64 {
    let Some(path) = (unsafe { path_bytes(path) }) else {
        return BAD_PATH;
    };
    if out.is_null() || len == 0 {
        return INVALID;
    }
    let node = match lookup(path) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let node = unsafe { &*node };
    if !node.directory {
        return NOT_DIR;
    }
    let out = unsafe { slice::from_raw_parts_mut(out, len) };
    let mut pos = 0;
    let mut child = node.child;
    while let Some(entry) = unsafe { child.as_ref() } {
        if pos + entry.name_len + 2 > out.len() {
            return NO_SPACE;
        }
        out[pos..pos + entry.name_len].copy_from_slice(&entry.name[..entry.name_len]);
        pos += entry.name_len;
        out[pos] = b'\n';
        pos += 1;
        child = entry.sibling;
    }
    out[pos] = 0;
    pos as i64
}

fn file(path: &[u8]) -> Result<*mut Node, i64> {
    let node = lookup(path)?;
    if unsafe { (*node).directory } {
        Err(IS_DIR)
    } else {
        Ok(node)
    }
}

unsafe fn slot(node: *mut Node, index: usize, create: bool) -> Option<*mut *mut u8> {
    unsafe {
        let mut slot = &raw mut (*node).pages;
        for group_index in 0..=index / SLOTS {
            if (*slot).is_null() {
                if !create {
                    return None;
                }
                *slot = allocate_page().cast();
                if (*slot).is_null() {
                    return None;
                }
            }
            let group = *slot;
            if group_index == index / SLOTS {
                return Some(&raw mut (*group).data[index % SLOTS]);
            }
            slot = &raw mut (*group).next;
        }
    }
    None
}

#[unsafe(no_mangle)]
unsafe extern "C" fn ramfs_read(path: *const c_char, out: *mut u8, len: usize, offset: u64) -> i64 {
    let Some(path) = (unsafe { path_bytes(path) }) else {
        return BAD_PATH;
    };
    if len == 0 {
        return 0;
    }
    if out.is_null() || len > isize::MAX as usize {
        return INVALID;
    }
    read(path, unsafe { slice::from_raw_parts_mut(out, len) }, offset)
}

pub fn read(path: &[u8], out: &mut [u8], offset: u64) -> i64 {
    let node = match file(path) {
        Ok(node) => node,
        Err(error) => return error,
    };
    read_node(node, out, offset)
}

fn read_node(node: *mut Node, out: &mut [u8], offset: u64) -> i64 {
    let offset = offset as usize;
    let length = unsafe { (*node).size }
        .saturating_sub(offset)
        .min(out.len());
    let mut copied = 0;
    while copied < length {
        let position = offset + copied;
        let within_page = position % PAGE;
        let count = (PAGE - within_page).min(length - copied);
        let buffer = &mut out[copied..copied + count];
        match unsafe { slot(node, position / PAGE, false).map(|entry| *entry) } {
            Some(page) if !page.is_null() => unsafe {
                ptr::copy_nonoverlapping(page.add(within_page), buffer.as_mut_ptr(), count);
            },
            _ => buffer.fill(0),
        }
        copied += count;
    }
    copied as i64
}

#[unsafe(no_mangle)]
unsafe extern "C" fn ramfs_write(
    path: *const c_char,
    data: *const u8,
    len: usize,
    offset: u64,
) -> i64 {
    let Some(path) = (unsafe { path_bytes(path) }) else {
        return BAD_PATH;
    };
    if len == 0 {
        return 0;
    }
    if data.is_null() || len > MAX_FILE {
        return INVALID;
    }
    write(path, unsafe { slice::from_raw_parts(data, len) }, offset)
}

pub fn write(path: &[u8], data: &[u8], offset: u64) -> i64 {
    let node = match file(path) {
        Ok(node) => node,
        Err(error) => return error,
    };
    write_node(node, data, offset)
}

fn write_node(node: *mut Node, data: &[u8], offset: u64) -> i64 {
    if offset > MAX_FILE as u64 || data.len() > MAX_FILE - offset as usize {
        return NO_SPACE;
    }
    let mut written = 0;
    while written < data.len() {
        let position = offset as usize + written;
        let within_page = position % PAGE;
        let count = (PAGE - within_page).min(data.len() - written);
        unsafe {
            let Some(entry) = slot(node, position / PAGE, true) else {
                break;
            };
            if (*entry).is_null() {
                *entry = allocate_page();
                if (*entry).is_null() {
                    break;
                }
            }
            ptr::copy_nonoverlapping(data.as_ptr().add(written), (*entry).add(within_page), count);
            written += count;
            (*node).size = (*node).size.max(offset as usize + written);
        }
    }
    if written == 0 && !data.is_empty() {
        NO_SPACE
    } else {
        written as i64
    }
}

pub fn truncate(path: &[u8], size: u64) -> i64 {
    let node = match file(path) {
        Ok(node) => node,
        Err(error) => return error,
    };
    truncate_node(node, size)
}

fn truncate_node(node: *mut Node, size: u64) -> i64 {
    if size > MAX_FILE as u64 {
        return NO_SPACE;
    }
    unsafe {
        let size = size as usize;
        let old = (*node).size;
        if size < old {
            let tail = size % PAGE;
            if tail != 0
                && let Some(entry) = slot(node, size / PAGE, false)
                && !(*entry).is_null()
            {
                (*entry).add(tail).write_bytes(0, PAGE - tail);
            }
            for index in size.div_ceil(PAGE)..old.div_ceil(PAGE) {
                if let Some(entry) = slot(node, index, false) {
                    bindings::kpfree(*entry);
                    *entry = ptr::null_mut();
                }
            }
        }
        (*node).size = size;
    }
    0
}

pub fn unlink(path: &[u8]) -> i64 {
    remove(path, false)
}

pub fn remove(path: &[u8], directory: bool) -> i64 {
    let (dir, name) = match parent(path) {
        Ok(value) => value,
        Err(error) => return error,
    };
    unsafe {
        let mut position = &raw mut (*dir).child;
        while let Some(node) = (*position).as_mut() {
            if node.name[..node.name_len] != *name {
                position = &raw mut node.sibling;
                continue;
            }
            if node.directory != directory {
                return if directory { NOT_DIR } else { IS_DIR };
            }
            if !node.child.is_null() {
                return -8;
            }

            let removed = *position;
            *position = node.sibling;
            forget_link(removed);
            return 0;
        }
    }
    MISSING
}

unsafe fn forget_link(node: *mut Node) {
    unsafe {
        (*node).linked = false;
        if (*node).references == 0 {
            destroy(node);
        }
    }
}

unsafe fn destroy(node: *mut Node) {
    unsafe {
        let mut pages = (*node).pages;
        while let Some(group) = pages.as_ref() {
            let next = group.next;
            for &page in &group.data {
                bindings::kpfree(page);
            }
            bindings::kpfree(pages.cast());
            pages = next;
        }
        bindings::kfree(node.cast());
    }
}

pub fn rename(from: &[u8], to: &[u8]) -> i64 {
    let (old_parent, old_name) = match parent(from) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let (new_parent, new_name) = match parent(to) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let source = match child(old_parent, old_name) {
        Ok(value) => value,
        Err(error) => return error,
    };
    if from == to {
        return 0;
    }
    let directory = unsafe { (*source).directory };
    if directory && to.starts_with(from) && to.get(from.len()) == Some(&b'/') {
        return INVALID;
    }
    if child(new_parent, new_name).is_ok() {
        let result = remove(to, directory);
        if result != 0 {
            return result;
        }
    }
    unsafe {
        let mut link = &raw mut (*old_parent).child;
        while *link != source {
            link = &raw mut (**link).sibling;
        }
        *link = (*source).sibling;
        (*source).sibling = (*new_parent).child;
        (*source).name_len = new_name.len();
        (*source).parent_inode = (*new_parent).inode;
        (*source)
            .name
            .as_mut_ptr()
            .copy_from_nonoverlapping(new_name.as_ptr(), new_name.len());
        (*new_parent).child = source;
    }
    0
}

#[unsafe(no_mangle)]
extern "C" fn lumen_ramfs_test() {
    const SOURCE: &[u8] = b"/.ramfs-check";
    const TARGET: &[u8] = b"/.ramfs-check-renamed";
    let offset = (SLOTS * PAGE - 2) as u64;
    let mut buffer = [0xff; 6];
    let passed = create(SOURCE, false) == 0
        && write(SOURCE, b"abcd", offset) == 4
        && open(SOURCE).is_ok_and(|handle| {
            rename(SOURCE, TARGET) == 0
                && unlink(TARGET) == 0
                && stat(SOURCE).is_err()
                && stat(TARGET).is_err()
                && handle.read(&mut buffer, offset - 2) == 6
                && buffer == *b"\0\0abcd"
                && handle.truncate(offset + 2) == 0
                && handle.truncate(offset + 4) == 0
                && handle.read(&mut buffer, offset - 2) == 6
                && buffer == *b"\0\0ab\0\0"
        });
    let _ = unlink(SOURCE);
    let _ = unlink(TARGET);
    let mut console = unsafe { crate::console::BootConsole::new() };
    if passed {
        console.write("[ SUCC ] RAMFS sparse file and open handle test passed.\n");
    } else {
        console.write("[ FERR ] RAMFS sparse file and open handle test failed.\n");
    }
}
