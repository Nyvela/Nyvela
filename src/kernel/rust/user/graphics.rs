use lumgfx::Display;

use super::memory::{self, NX, PAGE, PRESENT, WRITE};
use crate::{bindings, pci};

const WIDTH: u16 = 640;
const HEIGHT: u16 = 400;
const FRAME_SIZE: usize = WIDTH as usize * HEIGHT as usize * 4;
const FRAMEBUFFER: u64 = 0xffff_8001_0000_0000;
const CAPACITY: usize = 64;

const START: u64 = 0x4c00;
const DRAW: u64 = 0x4c01;
const KEY: u64 = 0x4c02;
const STOP: u64 = 0x4c03;

struct Graphics {
    owner: u64,
    display: Option<Display>,
    head: usize,
    tail: usize,
    extended: bool,
    keys: [u16; CAPACITY],
}

static mut GRAPHICS: Graphics = Graphics {
    owner: 0,
    display: None,
    head: 0,
    tail: 0,
    extended: false,
    keys: [0; CAPACITY],
};

#[unsafe(no_mangle)]
extern "C" fn lumen_graphics_prepare() {
    let Some(pci) = pci::locate(0x1234, 0x1111) else {
        return;
    };
    let bar = pci.read(0x10);
    let physical = u64::from(bar & !15);
    if bar & 7 != 0 || physical == 0 || physical % PAGE != 0 {
        return;
    }

    pci.enable(2);
    let cr3 = unsafe { bindings::kernel_cr3 };
    for offset in (0..FRAME_SIZE as u64).step_by(PAGE as usize) {
        if !unsafe {
            bindings::kvmmap_at(
                cr3,
                FRAMEBUFFER + offset,
                physical + offset,
                PRESENT | WRITE | NX | 0x18,
            )
        } {
            for mapped in (0..offset).step_by(PAGE as usize) {
                unsafe { bindings::kvmunmap_at(cr3, FRAMEBUFFER + mapped) };
            }
            return;
        }
    }
}

pub fn ioctl(tid: u64, cr3: u64, request: u64, argument: u64) -> Option<i64> {
    let graphics = unsafe { &mut *(&raw mut GRAPHICS) };
    match request {
        START => {
            if graphics.owner != 0 {
                return Some(-16); // EBUSY
            }
            let kernel = unsafe { bindings::kernel_cr3 };
            if memory::entry(kernel, FRAMEBUFFER) & PRESENT == 0 {
                return Some(-19); // ENODEV
            }
            let display = unsafe { Display::enable(FRAMEBUFFER as *mut u8, WIDTH, HEIGHT) };
            if display.is_none() {
                return Some(-19);
            }
            graphics.display = display;
            graphics.head = 0;
            graphics.tail = 0;
            graphics.extended = false;
            graphics.owner = tid;
            Some(0)
        }
        DRAW | KEY | STOP if graphics.owner != tid => Some(-1),
        DRAW => {
            if !memory::valid(cr3, argument, FRAME_SIZE, 0) {
                return Some(-14);
            }
            let mut buffer = [0; 2048];
            let display = graphics.display.as_mut().unwrap();
            for offset in (0..FRAME_SIZE).step_by(buffer.len()) {
                let count = (FRAME_SIZE - offset).min(buffer.len());
                memory::copy_in(cr3, argument + offset as u64, &mut buffer[..count]);
                display.write(offset, &buffer[..count]);
            }
            Some(0)
        }
        KEY => {
            if graphics.head == graphics.tail {
                return Some(0);
            }
            let key = graphics.keys[graphics.tail];
            graphics.tail = (graphics.tail + 1) % CAPACITY;
            Some(i64::from(key))
        }
        STOP => {
            graphics.display = None;
            graphics.owner = 0;
            Some(0)
        }
        _ => None,
    }
}

pub fn release(tid: u64) {
    let graphics = unsafe { &mut *(&raw mut GRAPHICS) };
    if graphics.owner == tid {
        graphics.display = None;
        graphics.owner = 0;
    }
}

#[unsafe(no_mangle)]
extern "C" fn lumen_graphics_key(code: u8) -> bool {
    let graphics = unsafe { &mut *(&raw mut GRAPHICS) };
    if graphics.owner == 0 {
        return false;
    }
    if code == 0xe0 {
        graphics.extended = true;
        return true;
    }
    let next = (graphics.head + 1) % CAPACITY;
    let extended = core::mem::take(&mut graphics.extended);
    if next != graphics.tail {
        graphics.keys[graphics.head] = u16::from(code & 0x7f)
            | (if code & 0x80 == 0 { 0x100 } else { 0 })
            | (if extended { 0x200 } else { 0 });
        graphics.head = next;
    }
    true
}
