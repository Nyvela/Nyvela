//! Foreground and background process lifetimes. Only the boot CPU schedules
//! userspace. Registry access happens with interrupts disabled.
use core::{
    ffi::{CStr, c_char},
    ptr, slice,
};

use crate::{abi, bindings, ramfs};

use super::{
    DEAD, Thread, accounts, arch, current_thread, elf, free_thread, graphics, memory, spawn_thread,
    syscalls::Process, threads, threads_length,
};

const MAX_PROCESSES: usize = 24;
const ARGS_MAX: usize = abi::ARGUMENT_LIMIT;
const PATH_MAX: usize = abi::PATH_LIMIT;

#[derive(Clone, Copy)]
pub struct Identity {
    pub account: abi::Account,
    pub parent: u64,
    pub seat: bool,
    pub captured: bool,
}

#[derive(Clone, Copy)]
struct Entry {
    tid: u64,
    thread: *mut Thread,
    identity: Identity,
}

static mut ENTRIES: [Option<Entry>; MAX_PROCESSES] = [None; MAX_PROCESSES];

pub fn identity(tid: u64) -> Option<Identity> {
    find(tid).map(|entry| entry.identity)
}

fn find(tid: u64) -> Option<Entry> {
    let entries = unsafe { &*(&raw const ENTRIES) };
    entries
        .iter()
        .flatten()
        .find(|entry| entry.tid == tid)
        .copied()
}

fn spawn(path: &CStr, args: &[u8], identity: Identity) -> Result<u64, i64> {
    let entries = unsafe { &mut *(&raw mut ENTRIES) };
    let slot = entries
        .iter_mut()
        .find(|entry| entry.is_none())
        .ok_or(-11_i64)?;

    let program = path.to_bytes();
    let (directory, size) = ramfs::stat(program).map_err(super::syscalls::fs_error)?;
    if directory || size == 0 {
        return Err(-8);
    }
    let mut magic = [0; 4];
    if ramfs::read(program, &mut magic, 0) != 4 || magic != *b"\x7fELF" {
        return Err(-8);
    }
    let image = unsafe {
        elf::load(
            path.as_ptr(),
            path.to_bytes_with_nul(),
            args,
            size as u64,
            identity.account,
        )
    }
    .map_err(|error| if error == -6 { -12 } else { -8 })?;

    let process = unsafe { bindings::kmalloc(size_of::<Process>() as u64) }.cast::<Process>();
    if process.is_null() {
        return Err(-12);
    }

    let child = unsafe { spawn_thread(image.entry, image.space.0) };
    if child.is_null() {
        unsafe { bindings::kfree(process.cast()) };
        return Err(-12);
    }
    unsafe {
        (*child).process = process.cast();
        (*(*child).context).cs = 0x1b;
        (*(*child).context).ss = 0x23;
        (*(*child).context).registers[7] = image.stack;
        process.write(Process::new(image.space, image.brk, identity));
        let tid = (*child).tid;
        *slot = Some(Entry {
            tid,
            thread: child,
            identity,
        });
        Ok(tid)
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn lumen_exec_elf(
    path: *const c_char,
    path_len: usize,
    arguments: *const u8,
    arguments_len: usize,
    _size: u64,
) -> i64 {
    let name = unsafe { slice::from_raw_parts(path.cast(), path_len) };
    let args = unsafe { slice::from_raw_parts(arguments, arguments_len) };
    let path = match CStr::from_bytes_with_nul(name) {
        Ok(path) => path,
        Err(_) => return -1,
    };
    let parent = unsafe { (*current_thread).tid };
    let identity = Identity {
        account: accounts::ROOT,
        parent,
        seat: true,
        captured: false,
    };
    match spawn(path, args, identity) {
        Ok(child) => wait(parent, child, false),
        Err(-2) => -2,
        Err(-11 | -12) => -6,
        Err(_) => -1,
    }
}

pub fn spawn_from(
    parent: u64,
    caller: Identity,
    cr3: u64,
    name: &[u8],
    args_address: u64,
    args_len: u64,
    requested_uid: u64,
    flags: u64,
) -> i64 {
    if args_len > ARGS_MAX as u64 || name.len() > PATH_MAX || flags & !3 != 0 {
        return -22;
    }
    if caller.account.uid != 0
        && requested_uid != u64::MAX
        && requested_uid != u64::from(caller.account.uid)
    {
        return -1;
    }
    let account = if requested_uid == u64::MAX {
        caller.account
    } else {
        let Some(account) = u32::try_from(requested_uid).ok().and_then(accounts::lookup) else {
            return -2;
        };
        account
    };
    if flags & 1 != 0 && !caller.seat {
        return -1;
    }
    let mut path = [0; PATH_MAX + 1];
    path[..name.len()].copy_from_slice(name);
    let result = CStr::from_bytes_with_nul(&path[..name.len() + 1]);
    let Ok(path) = result else { return -22 };
    let mut args = [0; ARGS_MAX];
    let args = &mut args[..args_len as usize];
    if !memory::copy_in(cr3, args_address, args) {
        return -14;
    }
    match spawn(
        path,
        args,
        Identity {
            parent,
            account,
            seat: flags & 1 != 0,
            captured: flags & 2 != 0,
        },
    ) {
        Ok(tid) => tid as i64,
        Err(error) => error,
    }
}

pub fn wait(parent: u64, child: u64, nonblocking: bool) -> i64 {
    let Some(entry) = find(child).filter(|entry| entry.identity.parent == parent) else {
        return -10;
    };
    unsafe {
        while ptr::addr_of!((*entry.thread).state).read_volatile() != DEAD {
            if nonblocking {
                return -11;
            }
            arch::wait_for_interrupt();
        }
        let status = (*entry.thread).exit_code;
        reap(entry);
        status
    }
}

pub fn terminate_children(parent: u64) {
    loop {
        let child = {
            let entries = unsafe { &*(&raw const ENTRIES) };
            entries
                .iter()
                .flatten()
                .find(|entry| entry.identity.parent == parent)
                .copied()
        };
        let Some(child) = child else { break };
        terminate_children(child.tid);
        graphics::release(child.tid);
        unsafe {
            reap(child);
        }
    }
}

unsafe fn reap(entry: Entry) {
    let child = entry.thread;
    let entries = unsafe { &mut *(&raw mut ENTRIES) };
    if let Some(slot) = entries
        .iter_mut()
        .find(|slot| slot.is_some_and(|value| value.tid == entry.tid))
    {
        *slot = None;
    }
    unsafe {
        let count = threads_length;
        for i in 0..count {
            if *threads.add(i) == child {
                ptr::copy(threads.add(i + 1), threads.add(i), count - i - 1);
                threads_length = count - 1;
                break;
            }
        }
        let process = (*child).process.cast::<Process>();
        process.drop_in_place();
        bindings::kfree(process.cast());
        free_thread(child);
    }
}

#[repr(C)]
struct Message {
    target: u64,
    sender: u64,
    bytes: *mut u8,
    length: u64,
}

unsafe extern "C" {
    fn kipc_alloc_payload(size: u64) -> *mut u8;
    fn kipc_free_message(message: *const Message);
    fn kipc_send(message: Message) -> bool;
    fn kipc_poll(message: *mut Message) -> bool;
}

pub fn send(tid: u64, caller: Identity, cr3: u64, target: u64, address: u64, length: u64) -> i64 {
    if length == 0 || length > abi::IPC_MAX as u64 {
        return -22;
    }
    let permitted = match identity(target) {
        Some(target) => caller.account.uid == 0 || caller.account.uid == target.account.uid,
        None => caller.account.uid == 0,
    };
    if !permitted {
        return -13;
    }
    let bytes = unsafe { kipc_alloc_payload(length) };
    if bytes.is_null() {
        return -12;
    }
    let message = Message {
        target,
        sender: tid,
        bytes,
        length,
    };
    let data = unsafe { slice::from_raw_parts_mut(bytes, length as usize) };
    if !memory::copy_in(cr3, address, data) {
        unsafe {
            kipc_free_message(&message);
        }
        return -14;
    }
    let copy = Message { ..message };
    if unsafe { kipc_send(message) } {
        1
    } else {
        unsafe {
            kipc_free_message(&copy);
        }
        -3
    }
}

pub fn receive(cr3: u64, address: u64, capacity: u64, sender: u64) -> i64 {
    let mut message = core::mem::MaybeUninit::<Message>::uninit();
    if !unsafe { kipc_poll(message.as_mut_ptr()) } {
        return 0;
    }
    let message = unsafe { message.assume_init() };
    let length = message.length as usize;
    let bytes = unsafe { slice::from_raw_parts(message.bytes, length) };
    let result = if message.length > capacity {
        -90
    } else if !memory::copy_out(cr3, address, bytes)
        || (sender != 0 && !memory::copy_out(cr3, sender, &message.sender.to_ne_bytes()))
    {
        -14
    } else {
        message.length as i64
    };
    unsafe {
        kipc_free_message(&message);
    }
    result
}
