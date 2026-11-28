mod arch;
mod elf;
mod graphics;
mod memory;
mod network;
mod syscalls;

use core::{ffi::c_char, ptr, slice};

use crate::bindings;

const DEAD: u32 = 3;

#[repr(C)]
pub struct Context {
    registers: [u64; 16],
    rip: u64,
    rflags: u64,
    cs: u64,
    ss: u64,
    cr3: u64,
    fs_base: u64,
    fx_state: u64,
}

#[repr(C)]
pub struct Thread {
    tid: u64,
    context: *mut Context,
    kernel_stack: *mut u8,
    state: u32,
    queue: *mut u8,
    exit_code: i64,
    process: *mut u8,
}

unsafe extern "C" {
    static mut current_thread: *mut Thread;
    static mut threads: *mut *mut Thread;
    static mut threads_length: usize;
    fn spawn_thread(entry: u64, cr3: u64) -> *mut Thread;
    fn free_thread(thread: *mut Thread);
    fn scheduler_tick() -> !;
}

#[unsafe(no_mangle)]
extern "C" fn lumen_user_memory_test() {
    let mut console = unsafe { crate::console::BootConsole::new() };
    if memory::self_test().is_some() {
        console.write("[ SUCC ] ELF address space and page permissions test passed.\n");
    } else {
        console.write("[ FERR ] ELF address space and page permissions test failed.\n");
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn lumen_exec_elf(
    path: *const c_char,
    path_len: usize,
    arguments: *const u8,
    arguments_len: usize,
    size: u64,
) -> i64 {
    let name = unsafe { slice::from_raw_parts(path.cast(), path_len) };
    let args = unsafe { slice::from_raw_parts(arguments, arguments_len) };
    let image = match unsafe { elf::load(path, name, args, size) } {
        Ok(image) => image,
        Err(error) => return error,
    };
    let process = unsafe { bindings::kmalloc(size_of::<syscalls::Process>() as u64) }
        .cast::<syscalls::Process>();
    if process.is_null() {
        return -6;
    }

    let child = unsafe { spawn_thread(image.entry, image.space.0) };
    if child.is_null() {
        unsafe { bindings::kfree(process.cast()) };
        return -6;
    }
    unsafe {
        (*child).process = process.cast();
        (*(*child).context).cs = 0x1b;
        (*(*child).context).ss = 0x23;
        (*(*child).context).registers[7] = image.stack;
        process.write(syscalls::Process::new(image.space, image.brk));

        while ptr::addr_of!((*child).state).read_volatile() != DEAD {
            arch::wait_for_interrupt();
        }

        let status = (*child).exit_code;
        let count = threads_length;
        for i in 0..count {
            if *threads.add(i) == child {
                ptr::copy(threads.add(i + 1), threads.add(i), count - i - 1);
                threads_length = count - 1;
                break;
            }
        }
        process.drop_in_place();
        bindings::kfree(process.cast());
        free_thread(child);
        status
    }
}

pub fn exit(status: i64) -> ! {
    unsafe {
        let current = current_thread;
        graphics::release((*current).tid);
        (*current).exit_code = status;
        (*current).state = DEAD;
        scheduler_tick();
    }
}

/// Lumen represents a synchronous fatal user exception as 128 + signal.
#[unsafe(no_mangle)]
extern "C" fn lumen_user_fault(vector: u64, _error: u64, _has_error: u64, frame: *const u64) -> ! {
    use core::fmt::Write;
    let signal = match vector {
        0 | 16 | 19 => 8, // SIGFPE
        1 | 3 => 5,       // SIGTRAP
        6 => 4,           // SIGILL
        17 => 7,          // SIGBUS
        _ => 11,          // SIGSEGV
    };
    let mut console = unsafe { crate::console::BootConsole::new() };
    let tid = unsafe { (*current_thread).tid };
    let rip = unsafe { frame.read() };
    let _ = writeln!(
        &mut console,
        "User process {tid} faulted at {rip:#x}, vector {vector}"
    );
    exit(128 + signal)
}
