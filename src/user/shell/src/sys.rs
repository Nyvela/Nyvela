//! Nyvela user-space syscall bindings (`int $0x80`).
//!
//! Numbers must match `include/nyvela/syscall/syscall.h`.
//! freestanding: `core` only, no allocator, no libc.

#![allow(dead_code)] // not every program uses every wrapper

pub const SYS_EXIT: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_YIELD: u64 = 2;
pub const SYS_READ: u64 = 3;

pub const SYS_FS_CREATE: u64 = 10;
pub const SYS_FS_WRITE: u64 = 11;
pub const SYS_FS_READ: u64 = 12;
pub const SYS_FS_LIST: u64 = 13;

pub const SYS_EXEC: u64 = 20;

/// Raw gate: rax = number, args = rdi, rsi, rdx, r10. Returns rax.
///
/// The kernel stub preserves every register except rax, but rcx/r11 are
/// listed clobbered defensively (`int` is opaque to the compiler).
#[inline(always)]
unsafe fn trap(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "int 0x80",
        inlateout("rax") nr => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub fn sys_exit(code: i64) -> ! {
    unsafe { trap(SYS_EXIT, code as u64, 0, 0, 0) };
    loop {}
}

#[inline(always)]
pub fn sys_write(fd: u64, buf: &[u8]) -> i64 {
    unsafe { trap(SYS_WRITE, fd, buf.as_ptr() as u64, buf.len() as u64, 0) as i64 }
}

#[inline(always)]
pub fn sys_read(fd: u64, buf: &mut [u8]) -> i64 {
    unsafe { trap(SYS_READ, fd, buf.as_mut_ptr() as u64, buf.len() as u64, 0) as i64 }
}

#[inline(always)]
pub fn sys_fs_list(path: &[u8], buf: &mut [u8]) -> i64 {
    unsafe {
        trap(
            SYS_FS_LIST,
            path.as_ptr() as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            0,
        ) as i64
    }
}

#[inline(always)]
pub fn sys_fs_read(path: &[u8], buf: &mut [u8], offset: u64) -> i64 {
    unsafe {
        trap(
            SYS_FS_READ,
            path.as_ptr() as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            offset,
        ) as i64
    }
}

#[inline(always)]
pub fn sys_exec(path: &[u8]) -> i64 {
    unsafe { trap(SYS_EXEC, path.as_ptr() as u64, 0, 0, 0) as i64 }
}

/// `strlen` for NUL-terminated byte strings.
pub fn slen(s: &[u8]) -> usize {
    let mut n = 0;
    while n < s.len() && s[n] != 0 {
        n += 1;
    }
    n
}

/// Decimal `itoa`, returns bytes written (excl. NUL, none added).
pub fn itoa(val: i64, out: &mut [u8]) -> usize {
    if out.is_empty() {
        return 0;
    }
    let neg = val < 0;
    // abs() without overflow on i64::MIN
    let mut u = if neg {
        (val as u64).wrapping_neg()
    } else {
        val as u64
    };
    let mut tmp = [0u8; 20];
    let mut t = 0;
    loop {
        tmp[t] = b'0' + (u % 10) as u8;
        t += 1;
        u /= 10;
        if u == 0 {
            break;
        }
    }
    let mut n = 0;
    if neg && n < out.len() {
        out[n] = b'-';
        n += 1;
    }
    while t > 0 && n < out.len() {
        t -= 1;
        out[n] = tmp[t];
        n += 1;
    }
    n
}
