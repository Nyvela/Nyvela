use crate::{acpi, bindings, console::BootConsole};
use core::{
    arch::{asm, global_asm, x86_64::__cpuid},
    cell::UnsafeCell,
    fmt::Write,
    hint::spin_loop,
    mem::size_of,
    ptr,
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};

const LAPIC: usize = 0xffff_8000_fee0_0000;
const APIC_ID: usize = 0x020;
const TPR: usize = 0x080;
const SVR: usize = 0x0f0;
const ICR_LOW: usize = 0x300;
const ICR_HIGH: usize = 0x310;
const DELIVERY_PENDING: u32 = 1 << 12;
const LVT_MASKED: u32 = 1 << 16;

const AP_BASE: usize = 0x7000;
const AP_VECTOR: u32 = (AP_BASE >> 12) as u32;
const PARAMS: usize = AP_BASE + 0x3f0;
const STACKS: usize = AP_BASE + 0x400;
const STACK_PAGES: u64 = 2;
const PAGE_SIZE: usize = 4096;
const AP_CODE_SEGMENT: u16 = 0x18;
const WORK_INTERRUPT: u8 = 0xf0;
const SPURIOUS_INTERRUPT: u8 = 0xff;
const TRAMPOLINE: &[u8] = include_bytes!(env!("NYVELA_AP_BOOT"));
const _: () = assert!(TRAMPOLINE.len() == STACKS - AP_BASE + 256 * size_of::<usize>());

const OFFLINE: u8 = 0;
const IDLE: u8 = 1;
const QUEUED: u8 = 2;
const DONE: u8 = 3;

pub(crate) type Task = fn(u8, usize) -> usize;

struct Worker {
    state: AtomicU8,
    job: UnsafeCell<Option<(Task, usize)>>,
    result: AtomicUsize,
}

impl Worker {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(OFFLINE),
            job: UnsafeCell::new(None),
            result: AtomicUsize::new(0),
        }
    }
}

// The BSP owns job writes while idle. Each AP reads its own job only after a
// release/acquire handoff, and the BSP recycles the slot after completion.
unsafe impl Sync for Worker {}

static WORKERS: [Worker; 256] = [const { Worker::new() }; 256];

#[derive(Clone, Copy)]
#[repr(C)]
struct Gate {
    offset_low: u16,
    segment: u16,
    ist: u8,
    flags: u8,
    offset_middle: u16,
    offset_high: u32,
    reserved: u32,
}

impl Gate {
    const EMPTY: Self = Self {
        offset_low: 0,
        segment: 0,
        ist: 0,
        flags: 0,
        offset_middle: 0,
        offset_high: 0,
        reserved: 0,
    };

    fn interrupt(entry: unsafe extern "C" fn()) -> Self {
        let address = entry as usize;
        Self {
            offset_low: address as u16,
            segment: AP_CODE_SEGMENT,
            ist: 0,
            flags: 0x8e,
            offset_middle: (address >> 16) as u16,
            offset_high: (address >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    address: u64,
}

static mut AP_IDT: [Gate; 256] = [Gate::EMPTY; 256];

global_asm!(
    ".pushsection .text",
    "nyvela_ap_wake:",
    "push rax",
    "movabs rax, 0xffff8000fee000b0",
    "mov dword ptr [rax], 0",
    "pop rax",
    "nyvela_ap_spurious:",
    "iretq",
    "nyvela_ap_fault:",
    "cli",
    "2:",
    "hlt",
    "jmp 2b",
    ".popsection",
);

unsafe extern "C" {
    fn nyvela_ap_wake();
    fn nyvela_ap_spurious();
    fn nyvela_ap_fault();
}

fn lapic_read(register: usize) -> u32 {
    unsafe { ptr::read_volatile(ptr::without_provenance(LAPIC + register)) }
}

fn lapic_write(register: usize, value: u32) {
    unsafe { ptr::write_volatile(ptr::without_provenance_mut(LAPIC + register), value) }
}

fn icr_idle() -> bool {
    for _ in 0..100_000 {
        if lapic_read(ICR_LOW) & DELIVERY_PENDING == 0 {
            return true;
        }
        spin_loop();
    }
    false
}

fn send_ipi(id: u8, command: u32) -> bool {
    if !icr_idle() {
        return false;
    }
    lapic_write(ICR_HIGH, u32::from(id) << 24);
    lapic_write(ICR_LOW, command);
    icr_idle()
}

fn port_read(port: u16) -> u8 {
    let value;
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack)) };
    value
}

fn port_write(port: u16, value: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack)) };
}

fn delay_ms(ms: u16) -> bool {
    // PIT channel 2 provides a polled one-shot while BSP interrupts are off.
    let previous = port_read(0x61);
    port_write(0x61, previous & !3);
    port_write(0x43, 0xb0);
    let [low, high] = (1193 * ms).to_le_bytes();
    port_write(0x42, low);
    port_write(0x42, high);
    port_write(0x61, (previous & !2) | 1);

    let complete = (0..1_000_000).any(|_| port_read(0x61) & 0x20 != 0);
    port_write(0x61, previous);
    complete
}

fn wait_for(worker: &Worker, state: u8) -> bool {
    for _ in 0..100 {
        if worker.state.load(Ordering::Acquire) == state {
            return true;
        }
        if !delay_ms(1) {
            break;
        }
    }
    worker.state.load(Ordering::Acquire) == state
}

fn start_processor(id: u8) -> bool {
    // INIT assert, INIT deassert, then up to two startup IPIs.
    let worker = &WORKERS[usize::from(id)];
    if !send_ipi(id, 0xc500) || !delay_ms(10) || !send_ipi(id, 0x8500) || !delay_ms(1) {
        return false;
    }
    for _ in 0..2 {
        if !send_ipi(id, 0x4600 | AP_VECTOR) || !delay_ms(1) {
            return false;
        }
        if worker.state.load(Ordering::Acquire) == IDLE {
            return true;
        }
    }
    wait_for(worker, IDLE)
}

fn prepare_trampoline() {
    let mut gates = [Gate::interrupt(nyvela_ap_fault); 256];
    gates[usize::from(WORK_INTERRUPT)] = Gate::interrupt(nyvela_ap_wake);
    gates[usize::from(SPURIOUS_INTERRUPT)] = Gate::interrupt(nyvela_ap_spurious);
    // The BSP publishes the IDT and trampoline before the first startup IPI.
    unsafe {
        ptr::addr_of_mut!(AP_IDT).write(gates);
        ptr::copy_nonoverlapping(
            TRAMPOLINE.as_ptr(),
            ptr::with_exposed_provenance_mut(AP_BASE),
            TRAMPOLINE.len(),
        );
        ptr::with_exposed_provenance_mut::<u64>(PARAMS).write(bindings::kernel_cr3);
        ptr::with_exposed_provenance_mut::<usize>(PARAMS + 8)
            .write((nyvela_ap_main as *const ()).addr());
    }
}

fn queue_job(id: u8, task: Task, arg: usize) {
    let worker = &WORKERS[usize::from(id)];
    // The BSP is the sole producer, and the slot is idle here.
    unsafe { worker.job.get().write(Some((task, arg))) };
    worker.state.store(QUEUED, Ordering::Release);
    send_ipi(id, u32::from(WORK_INTERRUPT));
}

/// Dispatches one invocation to each idle application processor and
/// collects the results after all jobs have been published.
///
/// # Safety
///
/// Call only on the BSP with interrupts disabled. Tasks run with the kernel
/// page table and must be safe to execute concurrently. A timeout leaves the
/// slot reserved, and its task may still complete after this call returns.
pub(crate) unsafe fn run_on_workers(
    task: Task,
    arg: usize,
    mut complete: impl FnMut(u8, Option<usize>),
) {
    let mut scheduled = [false; 256];
    for (id, worker) in WORKERS.iter().enumerate() {
        if worker.state.load(Ordering::Acquire) == IDLE {
            scheduled[id] = true;
            queue_job(id as u8, task, arg);
        }
    }
    for (id, worker) in WORKERS.iter().enumerate() {
        if !scheduled[id] {
            continue;
        }
        let result = wait_for(worker, DONE).then(|| {
            let result = worker.result.load(Ordering::Relaxed);
            worker.state.store(IDLE, Ordering::Release);
            result
        });
        complete(id as u8, result);
    }
}

fn probe_local_apic(id: u8, seed: usize) -> usize {
    if lapic_read(APIC_ID) >> 24 == u32::from(id) {
        seed ^ usize::from(id)
    } else {
        usize::MAX
    }
}

extern "C" fn nyvela_ap_main(id: usize) -> ! {
    let worker = &WORKERS[id];
    let idtr = Idtr {
        limit: (size_of::<[Gate; 256]>() - 1) as u16,
        address: ptr::addr_of!(AP_IDT).addr() as u64,
    };
    unsafe { asm!("lidt [{}]", in(reg) &idtr, options(readonly, nostack)) };

    for register in [0x320, 0x330, 0x340, 0x350, 0x360, 0x370] {
        lapic_write(register, LVT_MASKED);
    }
    lapic_write(TPR, 0);
    lapic_write(
        SVR,
        (lapic_read(SVR) & !0xff) | 0x100 | u32::from(SPURIOUS_INTERRUPT),
    );
    worker.state.store(IDLE, Ordering::Release);

    loop {
        if worker.state.load(Ordering::Acquire) == QUEUED {
            // The acquire sees the BSP's complete job before we read it.
            if let Some((task, arg)) = unsafe { worker.job.get().read() } {
                worker.result.store(task(id as u8, arg), Ordering::Relaxed);
            }
            worker.state.store(DONE, Ordering::Release);
        }
        // Interrupts stay disabled from the queue check up to the atomic
        // enable-and-halt sequence, so a wakeup cannot be missed.
        unsafe { asm!("sti; hlt; cli", options(nostack)) };
    }
}

/// # Safety
///
/// The BSP must call this once with exclusive kernel access, before the
/// scheduler starts and after installing the identity and local APIC mappings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nyvela_smp_init() {
    let mut console = unsafe { BootConsole::new() };
    let Some(mut enabled) = acpi::enabled_apic_ids() else {
        console.write("SMP: firmware CPU list unavailable, using the boot processor\n");
        return;
    };
    let bsp = (__cpuid(1).ebx >> 24) as u8;
    enabled[usize::from(bsp)] = false;
    if !enabled.iter().any(|&cpu| cpu) {
        console.write("SMP: 1 CPU online, 0 AP jobs verified\n");
        return;
    }

    prepare_trampoline();
    let mut online = 1;
    for (index, &available) in enabled.iter().enumerate() {
        if !available {
            continue;
        }
        let id = index as u8;
        let stack = unsafe { bindings::kpalloc_contiguous(STACK_PAGES) };
        if stack.is_null() {
            let _ = writeln!(console, "SMP: no stack available for AP {id}");
            continue;
        }
        let top = stack.addr() + STACK_PAGES as usize * PAGE_SIZE;
        unsafe { ptr::with_exposed_provenance_mut::<usize>(STACKS + index * 8).write(top) };
        if start_processor(id) {
            online += 1;
        } else {
            // A delayed AP may still enter the trampoline and use this stack.
            let _ = writeln!(console, "SMP: AP {id} did not start");
        }
    }

    const SEED: usize = 0x6e79_7665;
    let mut verified = 0;
    unsafe {
        run_on_workers(probe_local_apic, SEED, |id, result| {
            if result == Some(SEED ^ usize::from(id)) {
                verified += 1;
            } else {
                let _ = writeln!(console, "SMP: AP {id} failed the work check");
            }
        });
    }
    let _ = writeln!(
        console,
        "SMP: {online} CPUs online, {verified} AP jobs verified"
    );
}
