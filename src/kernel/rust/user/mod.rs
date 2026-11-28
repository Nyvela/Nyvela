mod accounts;
mod arch;
mod elf;
mod graphics;
mod memory;
mod network;
mod process;
mod syscalls;

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

pub fn exit(status: i64) -> ! {
    unsafe {
        let current = current_thread;
        graphics::release((*current).tid);
        process::terminate_children((*current).tid);
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
