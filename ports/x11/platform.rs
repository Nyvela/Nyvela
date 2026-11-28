//! Native framebuffer, input and process hooks for the upstream X11 session.
#![no_std]

#[path = "../../sdk/abi.rs"]
mod abi;
#[path = "../../sdk/sys.rs"]
mod sys;

use core::{ffi::CStr, slice};

const WIDTH: usize = abi::DISPLAY_WIDTH;
const HEIGHT: usize = abi::DISPLAY_HEIGHT;

fn call(number: u64, a: u64, b: u64, c: u64, d: u64) -> i64 {
    unsafe { sys::call(number, [a, b, c, d, 0, 0]) }
}

#[unsafe(no_mangle)]
extern "C" fn lumen_x11_acquire() -> i64 {
    call(abi::DISPLAY_ACQUIRE, 0, 0, 0, 0)
}

#[unsafe(no_mangle)]
extern "C" fn lumen_x11_key() -> u32 {
    let key = call(abi::DISPLAY_KEY, 0, 0, 0, 0);
    if key <= 0 {
        return 0;
    }
    let code = key as u32 & 0x7f;
    // XKB's evdev keycodes are Linux input key numbers plus eight. PS/2
    // set 1 matches ordinary keys. Extended keys need their evdev mapping.
    let code = if key & 0x200 == 0 {
        code
    } else {
        match code {
            0x1c => 96,    // keypad Enter
            0x1d => 97,    // right Ctrl
            0x35 => 98,    // keypad divide
            0x37 => 99,    // Print Screen
            0x38 => 100,   // right Alt
            0x47 => 102,   // Home
            0x48 => 103,   // Up
            0x49 => 104,   // Page Up
            0x4b => 105,   // Left
            0x4d => 106,   // Right
            0x4f => 107,   // End
            0x50 => 108,   // Down
            0x51 => 109,   // Page Down
            0x52 => 110,   // Insert
            0x53 => 111,   // Delete
            0x5b => 125,   // left Super
            0x5c => 126,   // right Super
            0x5d => 127,   // Menu
            _ => return 1, // Ignored scancode. More events may be queued.
        }
    };
    (code + 8) | (key as u32 & 0x100)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn lumen_x11_pointer(dx: *mut i32, dy: *mut i32, buttons: *mut u32) -> i64 {
    let mut event = abi::Pointer::default();
    let result = call(
        abi::DISPLAY_POINTER,
        (&raw mut event).addr() as u64,
        0,
        0,
        0,
    );
    if result > 0 {
        unsafe {
            dx.write(i32::from(event.dx));
            dy.write(i32::from(event.dy));
            buttons.write(u32::from(event.buttons));
        }
    }
    result
}

static mut PRESENTED: [u32; WIDTH * HEIGHT] = [0; WIDTH * HEIGHT];
static mut NEEDS_PRESENT: bool = true;

#[unsafe(no_mangle)]
unsafe extern "C" fn lumen_x11_present(pixels: *const u32, stride: usize) {
    let mut changed = unsafe { NEEDS_PRESENT };
    let presented = unsafe { &mut *(&raw mut PRESENTED) };
    for (y, scanline) in presented.chunks_exact_mut(WIDTH).enumerate() {
        let source = unsafe { slice::from_raw_parts(pixels.add(y * stride), WIDTH) };
        if scanline != source {
            scanline.copy_from_slice(source);
            changed = true;
        }
    }
    if changed {
        let result = call(
            abi::DISPLAY_PRESENT,
            presented.as_ptr().addr() as u64,
            0,
            0,
            0,
        );
        unsafe { NEEDS_PRESENT = result != 0 };
    }
}

static mut CLIENTS: [u64; 16] = [0; 16];

#[unsafe(no_mangle)]
extern "C" fn lumen_x11_reap() {
    for child in unsafe { &mut *(&raw mut CLIENTS) } {
        if *child != 0 && call(abi::WAIT, *child, 1, 0, 0) != -11 {
            *child = 0;
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn lumen_x11_launch(command: *const core::ffi::c_char) -> i64 {
    lumen_x11_reap();
    let Some(child) = (unsafe { &mut *(&raw mut CLIENTS) })
        .iter_mut()
        .find(|pid| **pid == 0)
    else {
        return -11;
    };
    let command = unsafe { CStr::from_ptr(command) }.to_bytes();
    let mut words = command
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|part| !part.is_empty());
    let Some(program) = words.next() else {
        return -22;
    };
    if !program.starts_with(b"/bin/") || program.len() >= abi::PATH_LIMIT {
        return -22;
    }
    let mut path = [0; abi::PATH_LIMIT];
    path[..program.len()].copy_from_slice(program);
    let mut args = [0; abi::ARGUMENT_LIMIT];
    let display = b"-display\0:0\0";
    args[..display.len()].copy_from_slice(display);
    let mut length = display.len();
    for word in words {
        if word.len() >= args.len() - length {
            return -7;
        }
        args[length..length + word.len()].copy_from_slice(word);
        length += word.len() + 1;
    }
    let result = call(
        abi::SPAWN,
        path.as_ptr().addr() as u64,
        args.as_ptr().addr() as u64,
        length as u64,
        u64::MAX,
    );
    if result > 0 {
        *child = result as u64;
    }
    result
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    call(abi::EXIT, 127, 0, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}
