use lumgfx::Display;
use lumouse::{Decoder, Motion};

use super::memory::{self, NX, PAGE, PRESENT, WRITE};
use crate::{abi, bindings, pci};

const WIDTH: u16 = 640;
const HEIGHT: u16 = 400;
const FRAME_SIZE: usize = WIDTH as usize * HEIGHT as usize * 4;
const FRAMEBUFFER: u64 = 0xffff_8001_0000_0000;
const CAPACITY: usize = 64;

struct Graphics {
    owner: u64,
    display: Option<Display>,
    head: usize,
    tail: usize,
    extended: bool,
    keys: [u16; CAPACITY],
    pointer: [Motion; CAPACITY],
    pointer_head: usize,
    pointer_tail: usize,
}

static mut GRAPHICS: Graphics = Graphics {
    owner: 0,
    display: None,
    head: 0,
    tail: 0,
    extended: false,
    keys: [0; CAPACITY],
    pointer: [Motion {
        dx: 0,
        dy: 0,
        buttons: 0,
    }; CAPACITY],
    pointer_head: 0,
    pointer_tail: 0,
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

pub fn call(tid: u64, cr3: u64, request: u64, argument: u64) -> i64 {
    let graphics = unsafe { &mut *(&raw mut GRAPHICS) };
    match request {
        abi::DISPLAY_ACQUIRE => {
            if graphics.owner != 0 {
                return -16; // EBUSY
            }
            let kernel = unsafe { bindings::kernel_cr3 };
            if memory::entry(kernel, FRAMEBUFFER) & PRESENT == 0 {
                return -19; // ENODEV
            }
            let display = unsafe { Display::enable(FRAMEBUFFER as *mut u8, WIDTH, HEIGHT) };
            if display.is_none() {
                return -19;
            }
            graphics.display = display;
            graphics.head = 0;
            graphics.tail = 0;
            graphics.extended = false;
            graphics.pointer_head = 0;
            graphics.pointer_tail = 0;
            graphics.owner = tid;
            0
        }
        abi::DISPLAY_PRESENT | abi::DISPLAY_KEY | abi::DISPLAY_RELEASE | abi::DISPLAY_POINTER
            if graphics.owner != tid =>
        {
            -1
        }
        abi::DISPLAY_PRESENT => {
            if !memory::valid(cr3, argument, FRAME_SIZE, 0) {
                return -14;
            }
            let mut buffer = [0; 2048];
            let display = graphics.display.as_mut().unwrap();
            for offset in (0..FRAME_SIZE).step_by(buffer.len()) {
                let count = (FRAME_SIZE - offset).min(buffer.len());
                memory::copy_in(cr3, argument + offset as u64, &mut buffer[..count]);
                display.write(offset, &buffer[..count]);
            }
            0
        }
        abi::DISPLAY_KEY => {
            if graphics.head == graphics.tail {
                return 0;
            }
            let key = graphics.keys[graphics.tail];
            graphics.tail = (graphics.tail + 1) % CAPACITY;
            i64::from(key)
        }
        abi::DISPLAY_POINTER => {
            if graphics.pointer_head == graphics.pointer_tail {
                return 0;
            }
            let event = graphics.pointer[graphics.pointer_tail];
            let mut record = [0; size_of::<abi::Pointer>()];
            record[..2].copy_from_slice(&event.dx.to_ne_bytes());
            record[2..4].copy_from_slice(&event.dy.to_ne_bytes());
            record[4] = event.buttons;
            if !memory::copy_out(cr3, argument, &record) {
                return -14;
            }
            graphics.pointer_tail = (graphics.pointer_tail + 1) % CAPACITY;
            1
        }
        abi::DISPLAY_RELEASE => {
            graphics.display = None;
            graphics.owner = 0;
            0
        }
        _ => -38,
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

static mut MOUSE: Decoder = Decoder::new();

unsafe extern "C" {
    fn kpic_enable_irq(irq: u8);
}

#[unsafe(no_mangle)]
extern "C" fn lumen_mouse_start() {
    if unsafe { lumouse::enable() } {
        unsafe {
            kpic_enable_irq(2);
            kpic_enable_irq(12);
            crate::console::BootConsole::new().write("PS/2 pointer ready.\n");
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn lumen_graphics_mouse(code: u8) {
    let decoder = unsafe { &mut *(&raw mut MOUSE) };
    let Some(event) = decoder.push(code) else {
        return;
    };
    let graphics = unsafe { &mut *(&raw mut GRAPHICS) };
    if graphics.owner == 0 {
        return;
    }
    let next = (graphics.pointer_head + 1) % CAPACITY;
    if next != graphics.pointer_tail {
        graphics.pointer[graphics.pointer_head] = event;
        graphics.pointer_head = next;
    }
}
