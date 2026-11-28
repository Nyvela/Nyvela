use font8x8::{BASIC_FONTS, UnicodeFonts};
use std::io;

use crate::{abi, sys};

pub const WIDTH: i32 = abi::DISPLAY_WIDTH as i32;
pub const HEIGHT: i32 = abi::DISPLAY_HEIGHT as i32;
pub const TEXT: u32 = 0xe7edf6;
pub const MUTED: u32 = 0x9caac2;
pub const ACCENT: u32 = 0x8ccfff;
pub const FRAME: u32 = 0x19243a;
pub const FIELD: u32 = 0x263750;
pub const ERROR: u32 = 0xffadab;

#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

pub struct Screen {
    pixels: Vec<u32>,
    active: bool,
}

impl Screen {
    pub fn open() -> io::Result<Self> {
        let mut screen = Self {
            pixels: vec![0; (WIDTH * HEIGHT) as usize],
            active: false,
        };
        screen.acquire()?;
        Ok(screen)
    }
    pub fn acquire(&mut self) -> io::Result<()> {
        sys::checked(sys::call(abi::DISPLAY_ACQUIRE, [0; 6]))?;
        self.active = true;
        Ok(())
    }
    pub fn release(&mut self) {
        if self.active {
            sys::call(abi::DISPLAY_RELEASE, [0; 6]);
            self.active = false;
        }
    }
    pub fn present(&self) -> io::Result<()> {
        sys::checked(sys::call(
            abi::DISPLAY_PRESENT,
            [self.pixels.as_ptr().addr() as u64, 0, 0, 0, 0, 0],
        ))?;
        Ok(())
    }
    pub fn rect(&mut self, rect: Rect, color: u32) {
        let left = rect.x.clamp(0, WIDTH);
        let right = (rect.x + rect.w).clamp(left, WIDTH);
        for row in rect.y.clamp(0, HEIGHT)..(rect.y + rect.h).clamp(0, HEIGHT) {
            let base = row * WIDTH;
            self.pixels[(base + left) as usize..(base + right) as usize].fill(color);
        }
    }
    pub fn text(&mut self, mut x: i32, y: i32, text: &str, color: u32, columns: usize) {
        for character in text.chars().take(columns) {
            if let Some(rows) = BASIC_FONTS.get(character).or_else(|| BASIC_FONTS.get('?')) {
                for (row, bits) in rows.into_iter().enumerate() {
                    for col in 0..8 {
                        if bits & (1 << col) != 0 {
                            self.rect(Rect::new(x + col, y + row as i32, 1, 1), color);
                        }
                    }
                }
            }
            x += 8;
        }
    }
    pub fn background(&mut self) {
        for y in 0..HEIGHT {
            let blue = 36 + y / 17;
            let green = 23 + y / 24;
            self.rect(
                Rect::new(0, y, WIDTH, 1),
                0x0e0000 | (green as u32) << 8 | blue as u32,
            );
        }
        self.rect(Rect::new(0, 0, WIDTH, 3), ACCENT);
    }
    pub fn button(&mut self, rect: Rect, label: &str, primary: bool) {
        self.rect(rect, if primary { ACCENT } else { FIELD });
        self.text(
            rect.x + 8,
            rect.y + (rect.h - 8) / 2,
            label,
            if primary { 0x101929 } else { TEXT },
            ((rect.w - 8) / 8).max(0) as usize,
        );
    }
    pub fn cursor(&mut self, x: i32, y: i32) {
        for row in 0..11 {
            let width = (row / 2 + 1).min(5);
            self.rect(Rect::new(x + 1, y + row + 1, width, 1), 0x000000);
            self.rect(Rect::new(x, y + row, width, 1), TEXT);
        }
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.release();
    }
}

#[derive(Clone, Copy)]
pub enum Key {
    Char(char),
    Control(char),
    Enter,
    Backspace,
    Tab,
    Escape,
    Function(u8),
    Up,
    Down,
    Left,
    Right,
}

enum Raw {
    Key(Key),
    Modifier,
}

pub struct Input {
    shift: u8,
    control: bool,
    caps: bool,
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
}

impl Input {
    pub fn new() -> Self {
        Self {
            shift: 0,
            control: false,
            caps: false,
            x: WIDTH / 2,
            y: HEIGHT / 2,
            buttons: 0,
        }
    }
    pub fn pointer(&mut self) -> bool {
        let mut motion = abi::Pointer::default();
        let result = sys::call(
            abi::DISPLAY_POINTER,
            [(&raw mut motion).addr() as u64, 0, 0, 0, 0, 0],
        );
        if result != 1 {
            return false;
        }
        self.x = (self.x + i32::from(motion.dx)).clamp(0, WIDTH - 1);
        self.y = (self.y + i32::from(motion.dy)).clamp(0, HEIGHT - 1);
        self.buttons = motion.buttons;
        true
    }
    pub fn key(&mut self) -> Option<Key> {
        loop {
            let event = sys::call(abi::DISPLAY_KEY, [0; 6]);
            if event <= 0 {
                return None;
            }
            if let Raw::Key(key) = self.decode(event as u16) {
                return Some(key);
            }
        }
    }
    fn decode(&mut self, event: u16) -> Raw {
        let code = (event & 0x7f) as u8;
        let pressed = event & 0x100 != 0;
        let extended = event & 0x200 != 0;
        if code == 0x1d {
            self.control = pressed;
            return Raw::Modifier;
        }
        if !extended && (code == 0x2a || code == 0x36) {
            let bit = if code == 0x2a { 1 } else { 2 };
            self.shift = if pressed {
                self.shift | bit
            } else {
                self.shift & !bit
            };
            return Raw::Modifier;
        }
        if !pressed {
            return Raw::Modifier;
        }
        if extended {
            return Raw::Key(match code {
                0x48 => Key::Up,
                0x50 => Key::Down,
                0x4b => Key::Left,
                0x4d => Key::Right,
                0x1c => Key::Enter,
                _ => return Raw::Modifier,
            });
        }
        let special = match code {
            1 => Key::Escape,
            0xe => Key::Backspace,
            0xf => Key::Tab,
            0x1c => Key::Enter,
            0x3a => {
                self.caps = !self.caps;
                return Raw::Modifier;
            }
            0x3b..=0x44 => Key::Function(code - 0x3a),
            0x57..=0x58 => Key::Function(code - 0x4c),
            _ => {
                let (plain, shifted) = match code {
                    0x02..=0x0d => {
                        let i = (code - 2) as usize;
                        (b"1234567890-="[i], b"!@#$%^&*()_+"[i])
                    }
                    0x10..=0x1b => {
                        let i = (code - 0x10) as usize;
                        (b"qwertyuiop[]"[i], b"QWERTYUIOP{}"[i])
                    }
                    0x1e..=0x29 => {
                        let i = (code - 0x1e) as usize;
                        (b"asdfghjkl;'\x60"[i], b"ASDFGHJKL:\"~"[i])
                    }
                    0x2b => (b'\\', b'|'),
                    0x2c..=0x35 => {
                        let i = (code - 0x2c) as usize;
                        (b"zxcvbnm,./"[i], b"ZXCVBNM<>?"[i])
                    }
                    0x39 => (b' ', b' '),
                    _ => return Raw::Modifier,
                };
                if self.control {
                    Key::Control(char::from(plain))
                } else {
                    let uppercase = (self.shift != 0) ^ (self.caps && plain.is_ascii_lowercase());
                    Key::Char(char::from(if uppercase { shifted } else { plain }))
                }
            }
        };
        Raw::Key(special)
    }
}
