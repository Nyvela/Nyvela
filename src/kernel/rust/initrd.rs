use core::fmt::Write;

use lumfw::{File, Firmware};

use crate::{console::BootConsole, ramfs};

#[unsafe(no_mangle)]
extern "C" fn lumen_initrd() {
    let Some(mut firmware) = (unsafe { Firmware::probe() }) else {
        return;
    };
    let Some(mut file) = firmware.file(b"opt/lumen/initrd") else {
        return;
    };
    let mut console = unsafe { BootConsole::new() };
    match import(&mut file) {
        Some(files) => {
            let _ = writeln!(console, "Initrd: loaded {files} files");
        }
        None => console.write("Initrd: malformed archive or insufficient RAM\n"),
    }
    if firmware.file(b"opt/lumen/desktop").is_some() && ramfs::create(b"/.desktop", false) == 0 {
        ramfs::write(b"/.desktop", b"1", 0);
    }
}

fn import(file: &mut File<'_>) -> Option<usize> {
    let mut header = [0; 110];
    let mut path = [0; 257];
    let mut buffer = [0; 1024];
    let mut files = 0;

    while file.read_exact(&mut header) {
        if header[..6] != *b"070701" {
            return None;
        }
        let mode = hex(&header[14..22])?;
        let size = hex(&header[54..62])?;
        let length = hex(&header[94..102])? as usize;
        if length == 0 || length > 256 || size > file.remaining() {
            return None;
        }
        if !file.read_exact(&mut path[1..length + 1])
            || path[length] != 0
            || !padding(file, header.len() + length)
        {
            return None;
        }

        if path[1..length] == *b"TRAILER!!!" {
            return (size == 0).then_some(files);
        }

        path[0] = b'/';
        let name = &path[..length];
        if name == b"/." && mode & 0xf000 == 0x4000 && size == 0 {
            continue;
        }
        if name[1..].split(|&byte| byte == b'/').any(|segment| {
            segment.is_empty() || segment == b"." || segment == b".." || segment.contains(&0)
        }) {
            return None;
        }
        match mode & 0xf000 {
            0x4000 if size == 0 => {
                if !matches!(ramfs::create(name, true), 0 | -3) {
                    return None;
                }
            }
            0x8000 => {
                if ramfs::create(name, false) != 0 {
                    return None;
                }
                let mut offset = 0;
                while offset < size {
                    let count = ((size - offset) as usize).min(buffer.len());
                    if !file.read_exact(&mut buffer[..count])
                        || ramfs::write(name, &buffer[..count], offset as u64) != count as i64
                    {
                        return None;
                    }
                    offset += count as u32;
                }
                if !padding(file, size as usize) {
                    return None;
                }
                files += 1;
            }
            _ => return None,
        }
    }
    None
}

fn padding(file: &mut File<'_>, length: usize) -> bool {
    let mut bytes = [0; 3];
    file.read_exact(&mut bytes[..(4 - length % 4) % 4])
}

fn hex(bytes: &[u8]) -> Option<u32> {
    bytes.iter().try_fold(0, |value: u32, byte| {
        let digit = (*byte as char).to_digit(16)?;
        value.checked_mul(16)?.checked_add(digit)
    })
}
