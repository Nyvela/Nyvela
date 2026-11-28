//! The system interface for the musl port. Musl's internal operation IDs
//! and POSIX data structures are translated before crossing into Lumen.
#![no_std]

#[path = "../sdk/abi.rs"]
mod abi;
#[path = "../sdk/sys.rs"]
mod sys;

use core::{panic::PanicInfo, ptr, slice};

const ENOENT: i64 = -2;
const EFAULT: i64 = -14;
const EINVAL: i64 = -22;
const ENOSYS: i64 = -38;
const ENOTTY: i64 = -25;
const EAFNOSUPPORT: i64 = -97;

fn native(number: u64, a: u64, b: u64, c: u64, d: u64, e: u64) -> i64 {
    unsafe { sys::call(number, [a, b, c, d, e, 0]) }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    __lumen_musl_exit(127)
}

#[unsafe(no_mangle)]
extern "C" fn __lumen_musl_tls(address: u64) -> i64 {
    native(abi::SET_TLS, address, 0, 0, 0, 0)
}

#[unsafe(no_mangle)]
extern "C" fn __lumen_musl_exit(status: u64) -> ! {
    native(abi::EXIT, status & 255, 0, 0, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}

#[unsafe(no_mangle)]
extern "C" fn __lumen_musl_call(n: u64, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> i64 {
    match n {
        0 => native(abi::READ, a, b, c, 0, 0),
        1 => native(abi::WRITE, a, b, c, 0, 0),
        2 => open(abi::CURRENT_DIRECTORY, a, b),
        3 => native(abi::CLOSE, a, 0, 0, 0, 0),
        4 | 6 => stat(false, abi::CURRENT_DIRECTORY, a, b),
        5 => stat(true, a, 0, b),
        7 => native(abi::POLL, a, b, c, 0, 0),
        8 => native(abi::SEEK, a, b, c, 0, 0),
        9 if d & 0x20 != 0 && d & 3 == 2 => {
            native(abi::MAP_MEMORY, a, b, c, u64::from(d & 0x100010 != 0), 0)
        }
        10 => native(abi::PROTECT_MEMORY, a, b, c, 0, 0),
        11 => native(abi::UNMAP_MEMORY, a, b, 0, 0, 0),
        12 => native(abi::BREAK, a, 0, 0, 0, 0),
        13 | 14 | 28 | 131 => 0, // Single-threaded applications have no asynchronous signal handlers.
        16 => ENOTTY,
        17 => native(abi::READ_AT, a, b, c, d, 0),
        18 => native(abi::WRITE_AT, a, b, c, d, 0),
        19 | 20 => iov(n == 20, a, b, c),
        21 => access(abi::CURRENT_DIRECTORY, a),
        24 => native(abi::YIELD, 0, 0, 0, 0, 0),
        35 => sleep(a),
        39 | 186 | 218 => native(abi::SELF, 0, 0, 0, 0, 0),
        41 => socket(a, b, c),
        42 | 49 => address_call(
            if n == 42 {
                abi::CONNECT_SOCKET
            } else {
                abi::BIND_SOCKET
            },
            a,
            b,
            c,
        ),
        43 | 288 => socket_output(
            abi::ACCEPT_SOCKET,
            a,
            b,
            c,
            if n == 288 {
                u64::from(d & 0x800 != 0)
            } else {
                0
            },
        ),
        44 => socket_send(a, b, c, d, e, f),
        45 => socket_receive(a, b, c, d, e, f),
        48 if matches!(b, 1 | 2) => native(abi::CLOSE_SOCKET_WRITE, a, 0, 0, 0, 0),
        50 => native(abi::LISTEN_SOCKET, a, 0, 0, 0, 0),
        51 | 52 => socket_output(abi::SOCKET_ADDRESS, a, b, c, u64::from(n == 52)),
        54 | 55 => socket_option(n == 54, a, b, c, d, e),
        60 | 231 => __lumen_musl_exit(a),
        63 => uname(a),
        72 => handle_flags(a, b, c),
        74 | 75 => native(abi::SYNC_FILE, a, 0, 0, 0, 0),
        77 => native(abi::RESIZE_FILE, a, b, 0, 0, 0),
        79 => native(abi::WORKING_DIRECTORY, a, b, 0, 0, 0),
        82 => native(
            abi::MOVE_PATH,
            a,
            abi::CURRENT_DIRECTORY,
            b,
            abi::CURRENT_DIRECTORY,
            0,
        ),
        83 => native(abi::CREATE_DIRECTORY, a, abi::CURRENT_DIRECTORY, 0, 0, 0),
        84 | 87 => native(
            abi::REMOVE_PATH,
            a,
            abi::CURRENT_DIRECTORY,
            u64::from(n == 84),
            0,
            0,
        ),
        89 | 267 => ENOENT,
        96 => time(a, true),
        97 => resource(a, b),
        102 | 104 | 107 | 108 => native(abi::IDENTITY, 0, 0, 0, 0, 0).max(0),
        110 => native(abi::IDENTITY, 2, 0, 0, 0, 0).max(1),
        158 if a == 0x1002 => __lumen_musl_tls(b),
        158 if a == 0x1003 => native(abi::GET_TLS, b, 0, 0, 0, 0),
        202 if matches!(b & 0x7f, 1 | 10) => 0,
        202 if matches!(b & 0x7f, 0 | 9) && a != 0 && a % 4 == 0 => {
            if unsafe { (a as *const u32).read_volatile() } != c as u32 {
                -11
            } else {
                ENOSYS
            }
        }
        204 if b != 0 && c != 0 => {
            unsafe {
                (c as *mut u8).write(1);
            }
            1
        }
        217 => directory(a, b, c),
        228 if matches!(a, 0 | 1 | 4 | 5 | 6 | 7) => time(b, false),
        230 if b == 0 => sleep(c),
        257 => open(dir(a), b, c),
        258 => native(abi::CREATE_DIRECTORY, b, dir(a), 0, 0, 0),
        262 if d & 0x1000 != 0 && b != 0 && unsafe { (b as *const u8).read() } == 0 => {
            stat(true, a, 0, c)
        }
        262 => stat(false, dir(a), b, c),
        263 if c & !0x200 == 0 => native(abi::REMOVE_PATH, b, dir(a), u64::from(c != 0), 0, 0),
        264 => native(abi::MOVE_PATH, b, dir(a), d, dir(c), 0),
        269 | 439 => access(dir(a), b),
        271 => {
            let timeout = if c == 0 {
                -1
            } else {
                millis(c).unwrap_or(0).min(i32::MAX as u64) as i32
            };
            native(abi::POLL, a, b, timeout as u64, 0, 0)
        }
        302 if a == 0 && c == 0 => resource(b, d),
        318 if c & !3 == 0 => native(abi::RANDOM, a, b, 0, 0, 0),
        _ => ENOSYS,
    }
}

fn dir(fd: u64) -> u64 {
    if fd as i32 == -100 {
        abi::CURRENT_DIRECTORY
    } else {
        fd
    }
}

fn open(directory: u64, path: u64, flags: u64) -> i64 {
    let mut access = match flags & 3 {
        0 => abi::OPEN_READ,
        1 => abi::OPEN_WRITE,
        2 => abi::OPEN_READ | abi::OPEN_WRITE,
        _ => return EINVAL,
    };
    for (value, bit) in [
        (0x40, abi::OPEN_CREATE),
        (0x80, abi::OPEN_EXCLUSIVE),
        (0x200, abi::OPEN_TRUNCATE),
        (0x400, abi::OPEN_APPEND),
        (0x800, abi::OPEN_NONBLOCK),
        (0x10000, abi::OPEN_DIRECTORY),
    ] {
        if flags & value != 0 {
            access |= bit;
        }
    }
    native(abi::OPEN, path, access, directory, 0, 0)
}

fn access(directory: u64, path: u64) -> i64 {
    let mut info = abi::FileInfo::default();
    native(
        abi::INFO_PATH,
        path,
        (&raw mut info).addr() as u64,
        directory,
        0,
        0,
    )
}

fn stat(handle: bool, directory: u64, path: u64, output: u64) -> i64 {
    if output == 0 {
        return EFAULT;
    }
    let mut info = abi::FileInfo::default();
    let address = (&raw mut info).addr() as u64;
    let result = if handle {
        native(abi::INFO_HANDLE, directory, address, 0, 0, 0)
    } else {
        native(abi::INFO_PATH, path, address, directory, 0, 0)
    };
    if result < 0 {
        return result;
    }
    let mut bytes = [0_u8; 144];
    let mode: u32 = match info.kind {
        abi::FILE_DIRECTORY => 0o40755,
        abi::FILE_CONSOLE => 0o20666,
        abi::FILE_SOCKET => 0o140666,
        _ => 0o100644,
    };
    for (offset, value) in [
        (0, 1),
        (8, info.inode),
        (16, 1),
        (48, info.size),
        (56, 4096),
        (64, info.size.div_ceil(512)),
    ] {
        bytes[offset..offset + 8].copy_from_slice(&value.to_ne_bytes());
    }
    for (offset, value) in [(24, mode), (28, info.owner), (32, info.owner)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), output as *mut u8, bytes.len()) };
    0
}

fn directory(fd: u64, address: u64, length: u64) -> i64 {
    if address == 0 {
        return EFAULT;
    }
    let mut written = 0;
    // Leave space for the largest possible entry before advancing Lumen's cursor.
    if length < 88 {
        return EINVAL;
    }
    while length - written >= 88 {
        let mut entry = abi::DirectoryEntry {
            inode: 0,
            next: 0,
            kind: 0,
            length: 0,
            name: [0; 64],
        };
        let result = native(
            abi::READ_DIRECTORY,
            fd,
            (&raw mut entry).addr() as u64,
            0,
            0,
            0,
        );
        if result <= 0 {
            return if written == 0 { result } else { written as i64 };
        }
        let len = (entry.length as usize).min(63);
        let count = (20 + len).next_multiple_of(8);
        let mut record = [0_u8; 88];
        record[..8].copy_from_slice(&entry.inode.to_ne_bytes());
        record[8..16].copy_from_slice(&entry.next.to_ne_bytes());
        record[16..18].copy_from_slice(&(count as u16).to_ne_bytes());
        record[18] = if entry.kind == abi::FILE_DIRECTORY {
            4
        } else {
            8
        };
        record[19..19 + len].copy_from_slice(&entry.name[..len]);
        unsafe {
            ptr::copy_nonoverlapping(record.as_ptr(), (address + written) as *mut u8, count);
        }
        written += count as u64;
    }
    written as i64
}

fn iov(write: bool, fd: u64, address: u64, count: u64) -> i64 {
    if count > 1024 {
        return EINVAL;
    }
    if count == 0 {
        return 0;
    }
    if address == 0 {
        return EFAULT;
    }
    let vectors = unsafe { slice::from_raw_parts(address as *const [u64; 2], count as usize) };
    let mut total = 0;
    for &[base, length] in vectors {
        if length == 0 {
            continue;
        }
        let result = native(
            if write { abi::WRITE } else { abi::READ },
            fd,
            base,
            length,
            0,
            0,
        );
        if result < 0 {
            return if total == 0 { result } else { total };
        }
        total += result;
        if result as u64 != length || (!write && fd == 0) {
            break;
        }
    }
    total
}

fn millis(address: u64) -> Result<u64, i64> {
    if address == 0 {
        return Err(EFAULT);
    }
    let [sec, ns] = unsafe { (address as *const [i64; 2]).read_unaligned() };
    if sec < 0 || !(0..1_000_000_000).contains(&ns) {
        return Err(EINVAL);
    }
    Ok((sec as u64)
        .saturating_mul(1000)
        .saturating_add((ns as u64).div_ceil(1_000_000)))
}

fn sleep(address: u64) -> i64 {
    match millis(address) {
        Ok(ms) => native(abi::SLEEP, ms, 0, 0, 0, 0),
        Err(error) => error,
    }
}

fn time(address: u64, microseconds: bool) -> i64 {
    if address == 0 {
        return if microseconds { 0 } else { EFAULT };
    }
    let ticks = native(abi::UPTIME, 0, 0, 0, 0, 0);
    if ticks < 0 {
        return ticks;
    }
    let units = if microseconds { 1000 } else { 1_000_000 };
    unsafe {
        (address as *mut [i64; 2]).write_unaligned([ticks / 1000, ticks % 1000 * units]);
    }
    0
}

fn resource(resource: u64, output: u64) -> i64 {
    if output == 0 {
        return 0;
    }
    let limit: u64 = match resource {
        3 => 1024 * 1024,
        7 => 13,
        0..=15 => u64::MAX,
        _ => return EINVAL,
    };
    unsafe {
        (output as *mut [u64; 2]).write_unaligned([limit, limit]);
    }
    0
}

fn uname(output: u64) -> i64 {
    if output == 0 {
        return EFAULT;
    }
    let bytes = unsafe { slice::from_raw_parts_mut(output as *mut u8, 390) };
    bytes.fill(0);
    for (i, field) in [
        b"Lumen".as_slice(),
        b"lumen",
        b"0.2",
        b"Lumen native",
        b"x86_64",
        b"local",
    ]
    .into_iter()
    .enumerate()
    {
        bytes[i * 65..i * 65 + field.len()].copy_from_slice(field);
    }
    0
}

fn handle_flags(fd: u64, command: u64, argument: u64) -> i64 {
    match command {
        1 | 2 => {
            let status = native(abi::HANDLE_FLAGS, fd, 0, 0, 0, 0);
            if status < 0 { status } else { 0 }
        }
        3 => {
            let status = native(abi::HANDLE_FLAGS, fd, 0, 0, 0, 0);
            if status < 0 {
                return status;
            }
            let access = match status as u64 & 3 {
                1 => 0,
                2 => 1,
                _ => 2,
            };
            access
                | if status as u64 & abi::OPEN_APPEND != 0 {
                    0x400
                } else {
                    0
                }
                | if status as u64 & abi::OPEN_NONBLOCK != 0 {
                    0x800
                } else {
                    0
                }
        }
        4 => native(
            abi::HANDLE_FLAGS,
            fd,
            1,
            if argument & 0x400 != 0 {
                abi::OPEN_APPEND
            } else {
                0
            } | if argument & 0x800 != 0 {
                abi::OPEN_NONBLOCK
            } else {
                0
            },
            0,
            0,
        ),
        _ => ENOSYS,
    }
}

fn socket(family: u64, kind: u64, protocol: u64) -> i64 {
    if family != 2 {
        return EAFNOSUPPORT;
    }
    if !matches!((kind & 0xf, protocol), (1, 0 | 6) | (2, 0 | 17)) {
        return -93;
    }
    native(
        abi::OPEN_SOCKET,
        kind & 0xf,
        u64::from(kind & 0x800 != 0),
        0,
        0,
        0,
    )
}

fn endpoint(address: u64, length: u64) -> Result<abi::Endpoint, i64> {
    if address == 0 {
        return Err(EFAULT);
    }
    if length < 16 {
        return Err(EINVAL);
    }
    let bytes = unsafe { (address as *const [u8; 16]).read_unaligned() };
    if u16::from_ne_bytes([bytes[0], bytes[1]]) != 2 {
        return Err(EAFNOSUPPORT);
    }
    Ok(abi::Endpoint {
        ip: [bytes[4], bytes[5], bytes[6], bytes[7]],
        port: u16::from_be_bytes([bytes[2], bytes[3]]),
    })
}

fn endpoint_out(endpoint: abi::Endpoint, address: u64, length: u64) {
    let count = unsafe { (length as *const u32).read_unaligned() }.min(16);
    let mut bytes = [0_u8; 16];
    bytes[..2].copy_from_slice(&2_u16.to_ne_bytes());
    bytes[2..4].copy_from_slice(&endpoint.port.to_be_bytes());
    bytes[4..8].copy_from_slice(&endpoint.ip);
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, count as usize);
        (length as *mut u32).write_unaligned(16);
    }
}

fn address_call(operation: u64, fd: u64, address: u64, length: u64) -> i64 {
    match endpoint(address, length) {
        Ok(address) => native(operation, fd, (&raw const address).addr() as u64, 0, 0, 0),
        Err(error) => error,
    }
}

fn socket_output(operation: u64, fd: u64, address: u64, length: u64, flags: u64) -> i64 {
    if address != 0 && length == 0 {
        return EFAULT;
    }
    let mut item = abi::Endpoint::default();
    let output = if address == 0 {
        0
    } else {
        (&raw mut item).addr() as u64
    };
    let result = native(operation, fd, output, flags, 0, 0);
    if result >= 0 && address != 0 {
        endpoint_out(item, address, length);
    }
    result
}

fn socket_send(fd: u64, data: u64, size: u64, flags: u64, address: u64, length: u64) -> i64 {
    if flags & !(0x40 | 0x4000) != 0 {
        return -95;
    }
    let mode = u64::from(flags & 0x40 != 0);
    if address == 0 {
        return native(abi::SEND_SOCKET, fd, data, size, mode, 0);
    }
    match endpoint(address, length) {
        Ok(item) => native(
            abi::SEND_SOCKET,
            fd,
            data,
            size,
            mode,
            (&raw const item).addr() as u64,
        ),
        Err(error) => error,
    }
}

fn socket_receive(fd: u64, data: u64, size: u64, flags: u64, address: u64, length: u64) -> i64 {
    if flags & !0x40 != 0 {
        return -95;
    }
    if address != 0 && length == 0 {
        return EFAULT;
    }
    let mut item = abi::Endpoint::default();
    let output = if address == 0 {
        0
    } else {
        (&raw mut item).addr() as u64
    };
    let result = native(
        abi::RECEIVE_SOCKET,
        fd,
        data,
        size,
        u64::from(flags & 0x40 != 0),
        output,
    );
    if result >= 0 && address != 0 {
        endpoint_out(item, address, length);
    }
    result
}

fn socket_option(set: bool, fd: u64, level: u64, option: u64, value: u64, length: u64) -> i64 {
    if level != 1 {
        return -92;
    }
    let option = match option {
        2 => abi::SOCKET_REUSE,
        3 => abi::SOCKET_KIND,
        4 => abi::SOCKET_ERROR,
        20 | 66 => abi::SOCKET_READ_TIMEOUT,
        21 | 67 => abi::SOCKET_WRITE_TIMEOUT,
        _ => return -92,
    };
    if value == 0 || (!set && length == 0) {
        return EFAULT;
    }
    let timer = matches!(option, abi::SOCKET_READ_TIMEOUT | abi::SOCKET_WRITE_TIMEOUT);
    let min_size = if timer { 16 } else { 4 };
    if set {
        if length < min_size {
            return EINVAL;
        }
        let argument = if timer {
            let [sec, us] = unsafe { (value as *const [i64; 2]).read_unaligned() };
            if sec < 0 || !(0..1_000_000).contains(&us) {
                return EINVAL;
            }
            (sec as u64)
                .saturating_mul(1000)
                .saturating_add((us as u64).div_ceil(1000))
        } else {
            unsafe { (value as *const u32).read_unaligned() as u64 }
        };
        return native(abi::SOCKET_OPTION, fd, option, 1, argument, 0);
    }
    let result = native(abi::SOCKET_OPTION, fd, option, 0, 0, 0);
    if result < 0 {
        return result;
    }
    let values = if timer {
        [result / 1000, result % 1000 * 1000]
    } else {
        [result, 0]
    };
    let count = unsafe { (length as *const u32).read_unaligned() }.min(min_size as u32);
    unsafe {
        ptr::copy_nonoverlapping(values.as_ptr().cast(), value as *mut u8, count as usize);
        (length as *mut u32).write_unaligned(min_size as u32);
    }
    0
}
