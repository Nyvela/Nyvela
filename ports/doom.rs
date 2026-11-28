// SPDX-License-Identifier: GPL-2.0-or-later
#![no_std]

use core::ffi::{c_char, c_int, c_ulong, c_void};

const VIDEO_START: c_ulong = 0x4c00;
const VIDEO_DRAW: c_ulong = 0x4c01;
const VIDEO_KEY: c_ulong = 0x4c02;
const CONSOLE_OUT: c_int = 1;

#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}

unsafe extern "C" {
    static mut DG_ScreenBuffer: *mut u32;
    fn doomgeneric_Create(argc: c_int, argv: *mut *mut c_char);
    fn doomgeneric_Tick();
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn write(fd: c_int, buffer: *const c_void, size: usize) -> isize;
    fn exit(code: c_int) -> !;
    fn nanosleep(request: *const Timespec, remaining: *mut Timespec) -> c_int;
    fn clock_gettime(clock: c_int, time: *mut Timespec) -> c_int;
}

#[unsafe(no_mangle)]
extern "C" fn main(argc: c_int, argv: *mut *mut c_char) -> c_int {
    unsafe { doomgeneric_Create(argc, argv) };
    loop {
        unsafe { doomgeneric_Tick() };
    }
}

#[unsafe(no_mangle)]
extern "C" fn DG_Init() {
    unsafe {
        if ioctl(CONSOLE_OUT, VIDEO_START, 0_u64) < 0 {
            let message = b"Doom needs the QEMU Standard VGA device.\n";
            write(2, message.as_ptr().cast(), message.len());
            exit(1);
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn DG_DrawFrame() {
    unsafe { ioctl(CONSOLE_OUT, VIDEO_DRAW, DG_ScreenBuffer) };
}

#[unsafe(no_mangle)]
extern "C" fn DG_SleepMs(ms: u32) {
    let time = Timespec {
        sec: i64::from(ms / 1000),
        nsec: i64::from(ms % 1000) * 1_000_000,
    };
    unsafe { nanosleep(&time, core::ptr::null_mut()) };
}

#[unsafe(no_mangle)]
extern "C" fn DG_GetTicksMs() -> u32 {
    let mut time = Timespec { sec: 0, nsec: 0 };
    unsafe { clock_gettime(1, &mut time) };
    (time.sec as u32)
        .wrapping_mul(1000)
        .wrapping_add((time.nsec / 1_000_000) as u32)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn DG_GetKey(pressed: *mut c_int, key: *mut u8) -> c_int {
    loop {
        let event = unsafe { ioctl(CONSOLE_OUT, VIDEO_KEY, 0_u64) };
        if event <= 0 {
            return 0;
        }
        if let Some(value) = doom_key((event & 0x7f) as u8, event & 0x200 != 0) {
            unsafe {
                pressed.write(c_int::from(event & 0x100 != 0));
                key.write(value);
            }
            return 1;
        }
    }
}

fn doom_key(code: u8, extended: bool) -> Option<u8> {
    if extended {
        return Some(match code {
            0x48 => 0xad,
            0x4b => 0xac,
            0x4d => 0xae,
            0x50 => 0xaf,
            0x1c => 13,
            0x1d => 0xa3,
            0x38 => 0xb8,
            _ => return None,
        });
    }
    let value = match code {
        0x01 => 27,
        0x02..=0x0b => b"1234567890"[(code - 2) as usize],
        0x0c => b'-',
        0x0d => b'=',
        0x0e => 0x7f,
        0x0f => 9,
        0x10..=0x19 => b"qwertyuiop"[(code - 0x10) as usize],
        0x1c => 13,
        0x1d => 0xa3,
        0x1e..=0x26 => b"asdfghjkl"[(code - 0x1e) as usize],
        0x2a | 0x36 => 0xb6,
        0x2c..=0x32 => b"zxcvbnm"[(code - 0x2c) as usize],
        0x33 => b',',
        0x34 => b'.',
        0x38 => 0xb8,
        0x39 => 0xa2,
        0x3b..=0x44 | 0x57 | 0x58 => 0x80 + code,
        _ => return None,
    };
    Some(value)
}

#[unsafe(no_mangle)]
extern "C" fn DG_SetWindowTitle(_title: *const c_char) {}
