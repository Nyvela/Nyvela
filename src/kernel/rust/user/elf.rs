use core::ffi::c_char;

use super::{arch, memory};
use memory::{NX, PAGE, PRESENT, STACK_SIZE, STACK_TOP, Space, USER, WRITE};

const INVALID: i64 = -1;
const NO_SPACE: i64 = -6;
const ELF_HEADER_SIZE: usize = 64;
const PH_SIZE: u64 = 56;
const LOAD: u32 = 1;
const PHDR: u32 = 6;
const EXEC: u32 = 1;
const WRITABLE: u32 = 2;
const MAX_SEGMENTS: usize = 24;
const MAX_IMAGE: u64 = 64 * 1024 * 1024;

pub struct Executable {
    pub space: Space,
    pub entry: u64,
    pub stack: u64,
    pub brk: u64,
}

#[derive(Clone, Copy, Default)]
struct Segment {
    offset: u64,
    address: u64,
    file_size: u64,
    memory_size: u64,
    flags: u32,
}

impl Segment {
    fn end(self) -> u64 {
        self.address + self.memory_size
    }

    fn page_flags(self) -> u64 {
        let mut flags = 0;
        if self.flags & EXEC == 0 {
            flags |= NX;
        }
        if self.flags & WRITABLE != 0 {
            flags |= WRITE;
        }
        flags
    }
}

pub unsafe fn load(
    path: *const c_char,
    program_name: &[u8],
    arguments: &[u8],
    file_size: u64,
) -> Result<Executable, i64> {
    if file_size > MAX_IMAGE {
        return Err(NO_SPACE);
    }

    let mut header = [0; ELF_HEADER_SIZE];
    read(path, &mut header, 0)?;
    if header[..7] != *b"\x7fELF\x02\x01\x01"
        || u16_at(&header, 16) != 2 // ET_EXEC
        || u16_at(&header, 18) != 62 // EM_X86_64
        || u32_at(&header, 20) != 1
        || u16_at(&header, 52) as usize != ELF_HEADER_SIZE
        || u16_at(&header, 54) as u64 != PH_SIZE
    {
        return Err(INVALID);
    }

    let entry = u64_at(&header, 24);
    let ph_offset = u64_at(&header, 32);
    let ph_count = u16_at(&header, 56) as u64;
    let ph_end = ph_offset.checked_add(ph_count * PH_SIZE).ok_or(INVALID)?;
    if ph_count == 0 || ph_count > 128 || ph_end > file_size {
        return Err(INVALID);
    }

    let mut segments = [Segment::default(); MAX_SEGMENTS];
    let mut count = 0;
    let mut program_headers = None;
    let mut image_end = 0;
    let mut mapped_size = 0;
    let mut executable_entry = false;

    for i in 0..ph_count {
        let mut bytes = [0; PH_SIZE as usize];
        read(path, &mut bytes, ph_offset + i * PH_SIZE)?;
        match u32_at(&bytes, 0) {
            2 | 3 => return Err(INVALID), // PT_DYNAMIC, PT_INTERP
            PHDR => program_headers = Some(u64_at(&bytes, 16)),
            LOAD => {
                let segment = Segment {
                    flags: u32_at(&bytes, 4),
                    offset: u64_at(&bytes, 8),
                    address: u64_at(&bytes, 16),
                    file_size: u64_at(&bytes, 32),
                    memory_size: u64_at(&bytes, 40),
                };
                let align = u64_at(&bytes, 48);
                let end = segment
                    .address
                    .checked_add(segment.memory_size)
                    .ok_or(INVALID)?;
                let file_end = segment
                    .offset
                    .checked_add(segment.file_size)
                    .ok_or(INVALID)?;
                if segment.file_size > segment.memory_size
                    || file_end > file_size
                    || segment.address % PAGE != segment.offset % PAGE
                    || (align > 1
                        && (!align.is_power_of_two()
                            || segment.address % align != segment.offset % align))
                {
                    return Err(INVALID);
                }
                if segment.memory_size == 0 {
                    continue;
                }
                let first = memory::page_floor(segment.address);
                let last = memory::page_ceil(end).ok_or(INVALID)?;
                if !memory::image_range(first, last)
                    || count == MAX_SEGMENTS
                    || segments[..count]
                        .iter()
                        .any(|other| segment.address < other.end() && other.address < end)
                {
                    return Err(INVALID);
                }
                executable_entry |=
                    segment.flags & EXEC != 0 && (segment.address..end).contains(&entry);
                image_end = image_end.max(last);
                mapped_size += last - first;
                if mapped_size > MAX_IMAGE {
                    return Err(NO_SPACE);
                }
                segments[count] = segment;
                count += 1;
            }
            _ => {}
        }
    }
    if !executable_entry {
        return Err(INVALID);
    }

    // A static C runtime looks up its TLS template in the mapped PHDR table.
    let mapped_headers = segments[..count]
        .iter()
        .find_map(|segment| {
            if ph_offset >= segment.offset && ph_end <= segment.offset + segment.file_size {
                Some(segment.address + ph_offset - segment.offset)
            } else {
                None
            }
        })
        .ok_or(INVALID)?;
    if program_headers.is_some_and(|value| value != mapped_headers) {
        return Err(INVALID);
    }

    let space = Space::new().ok_or(NO_SPACE)?;
    let cr3 = space.0;
    for &segment in &segments[..count] {
        map_segment(cr3, path, segment)?;
    }
    // Leave a guard page at the bottom of the one MiB stack reservation.
    for address in ((STACK_TOP - STACK_SIZE + PAGE)..STACK_TOP).step_by(PAGE as usize) {
        if !memory::map_new(cr3, address, WRITE | NX) {
            return Err(NO_SPACE);
        }
    }

    let stack = initial_stack(
        cr3,
        program_name,
        arguments,
        entry,
        mapped_headers,
        ph_count,
    )?;
    Ok(Executable {
        space,
        entry,
        stack,
        brk: image_end,
    })
}

fn map_segment(cr3: u64, path: *const c_char, segment: Segment) -> Result<(), i64> {
    let first = memory::page_floor(segment.address);
    let end = memory::page_ceil(segment.end()).ok_or(INVALID)?;
    for address in (first..end).step_by(PAGE as usize) {
        let old = memory::entry(cr3, address);
        let mut flags = segment.page_flags();
        if old & (PRESENT | USER) == PRESENT | USER {
            flags = ((flags | old) & WRITE) | (flags & old & NX);
        }
        if flags & (WRITE | NX) == WRITE {
            return Err(INVALID);
        }
        let mapped = if old & (PRESENT | USER) == PRESENT | USER {
            memory::protect(cr3, address, flags)
        } else {
            memory::map_new(cr3, address, flags)
        };
        if !mapped {
            return Err(NO_SPACE);
        }
    }

    let mut buffer = [0; 1024];
    let mut copied = 0;
    while copied < segment.file_size {
        let count = (segment.file_size - copied).min(buffer.len() as u64) as usize;
        read(path, &mut buffer[..count], segment.offset + copied)?;
        if !memory::load_bytes(cr3, segment.address + copied, &buffer[..count]) {
            return Err(INVALID);
        }
        copied += count as u64;
    }
    Ok(())
}

fn initial_stack(
    cr3: u64,
    program_name: &[u8],
    arguments: &[u8],
    entry: u64,
    phdr: u64,
    ph_count: u64,
) -> Result<u64, i64> {
    let mut stack = STACK_TOP;
    let mut argv = [0; 32];
    argv[0] = push(cr3, &mut stack, program_name)?;
    let mut argc = 1;
    for argument in arguments.split_inclusive(|&byte| byte == 0) {
        let Some(slot) = argv.get_mut(argc) else {
            return Err(INVALID);
        };
        if argument.last() != Some(&0) {
            return Err(INVALID);
        }
        *slot = push(cr3, &mut stack, argument)?;
        argc += 1;
    }

    let mut seed = [0; 16];
    for (i, word) in seed.chunks_exact_mut(8).enumerate() {
        let entropy = arch::random().unwrap_or_else(|| arch::tsc().rotate_left(i as u32 * 17));
        word.copy_from_slice(&entropy.to_ne_bytes());
    }
    let random = push(cr3, &mut stack, &seed)?;
    let env = [
        push(cr3, &mut stack, b"HOME=/\0")?,
        push(cr3, &mut stack, b"PATH=/bin\0")?,
        push(cr3, &mut stack, b"TERM=dumb\0")?,
    ];
    let aux = [
        3, phdr, 4, PH_SIZE, 5, ph_count, 6, PAGE, 7, 0, 9, entry, 11, 1000, 12, 1000, 13, 1000,
        14, 1000, 17, 100, 23, 0, 25, random, 31, argv[0], 0, 0,
    ];
    let stack_words = 1 + argc + 2 + env.len() + aux.len();
    stack = (stack - 8 * stack_words as u64) & !15;
    let mut address = stack;

    for value in core::iter::once(argc as u64)
        .chain(argv[..argc].iter().copied())
        .chain([0])
        .chain(env)
        .chain([0])
        .chain(aux)
    {
        if !memory::copy_out(cr3, address, &value.to_ne_bytes()) {
            return Err(INVALID);
        }
        address += 8;
    }
    Ok(stack)
}

fn push(cr3: u64, stack: &mut u64, data: &[u8]) -> Result<u64, i64> {
    *stack = stack.checked_sub(data.len() as u64).ok_or(INVALID)?;
    if memory::copy_out(cr3, *stack, data) {
        Ok(*stack)
    } else {
        Err(INVALID)
    }
}

fn read(path: *const c_char, buffer: &mut [u8], offset: u64) -> Result<(), i64> {
    let got = unsafe {
        crate::bindings::vfs_read(path, buffer.as_mut_ptr(), buffer.len() as u64, offset)
    };
    if got == buffer.len() as i64 {
        Ok(())
    } else {
        Err(INVALID)
    }
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
