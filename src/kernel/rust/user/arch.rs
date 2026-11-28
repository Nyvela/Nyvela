use core::arch::{asm, x86_64::__cpuid};

const EFER: u32 = 0xc000_0080;
const FS_BASE: u32 = 0xc000_0100;

unsafe extern "C" {
    fn isr_elf_syscall();
}

#[unsafe(no_mangle)]
extern "C" fn lumen_arch_user_init() {
    unsafe {
        // x86-64 guarantees SSE2 and FXSAVE. Keep AVX disabled until the
        // scheduler supports an XSAVE context.
        asm!(
            "mov rax, cr0",
            "and rax, -13",
            "or rax, 0x10022",
            "mov cr0, rax",
            "mov rax, cr4",
            "or rax, 0x600",
            "mov cr4, rax",
            "fninit",
            out("rax") _,
            options(nostack),
        );
        let efer = read_msr(EFER);
        write_msr(EFER, efer | (1 << 11) | 1); // NX and SYSCALL
        write_msr(0xc000_0081, 0x28 << 32); // STAR: kernel CS
        write_msr(0xc000_0082, isr_elf_syscall as *const () as u64);
        write_msr(0xc000_0084, 0x44700); // TF, IF, DF, NT and AC
    }
}

#[unsafe(no_mangle)]
extern "C" fn lumen_idle() -> ! {
    loop {
        wait_for_interrupt();
        super::network::poll();
    }
}

pub fn wait_for_interrupt() {
    unsafe { asm!("sti", "hlt", "cli", options(nostack)) };
}

pub fn set_fs_base(base: u64) {
    unsafe { write_msr(FS_BASE, base) };
}

pub fn tsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

pub fn random() -> Option<u64> {
    if __cpuid(1).ecx & (1 << 30) == 0 {
        return None;
    }
    for _ in 0..10 {
        let value: u64;
        let success: u8;
        unsafe {
            asm!(
                "rdrand {value}",
                "setc {success}",
                value = out(reg) value,
                success = out(reg_byte) success,
                options(nomem, nostack),
            );
        }
        if success != 0 {
            return Some(value);
        }
    }
    None
}

unsafe fn read_msr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack));
    }
    (u64::from(hi) << 32) | u64::from(lo)
}

unsafe fn write_msr(msr: u32, value: u64) {
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nostack),
        );
    }
}
