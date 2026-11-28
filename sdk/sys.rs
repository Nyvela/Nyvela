//! Direct calls from native Lumen applications.
#![allow(dead_code)]

/// # Safety
///
/// Pointer arguments must follow the ownership and buffer contract of the
/// selected operation.
#[inline]
pub unsafe fn call(number: u64, args: [u64; 6]) -> i64 {
    let result: i64;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") number => result,
            in("rdi") args[0], in("rsi") args[1], in("rdx") args[2],
            in("r10") args[3], in("r8") args[4], in("r9") args[5],
            options(nostack),
        );
    }
    result
}
