//! Static x86-64 musl process ABI. Native Lumen services still use int 0x80.
use core::ptr;

use super::{Context, Thread, arch, current_thread, exit, memory, network};
use crate::{bindings, ramfs};
use memory::{MAP_END, MAP_FIRST, NO_ACCESS, NX, PAGE, PRESENT, Space, WRITE};

const EPERM: i64 = -1;
const ENOENT: i64 = -2;
const EBADF: i64 = -9;
const EAGAIN: i64 = -11;
const ENOMEM: i64 = -12;
const EACCES: i64 = -13;
const EFAULT: i64 = -14;
const EEXIST: i64 = -17;
const ENOTDIR: i64 = -20;
const EISDIR: i64 = -21;
const EINVAL: i64 = -22;
const EMFILE: i64 = -24;
const ENOTTY: i64 = -25;
const ENOSPC: i64 = -28;
const ESPIPE: i64 = -29;
const ENAMETOOLONG: i64 = -36;
const ENOSYS: i64 = -38;
const AT_FDCWD: u64 = -100_i64 as u64;
const PATH_MAX: usize = 256;
const FDS: usize = 10;
const IO_MAX: usize = 1024 * 1024;
const MAX_MAPPING: u64 = 64 * 1024 * 1024;

unsafe extern "C" {
    static lapic_ticks_ms: u64;
    fn kbd_getc() -> i32;
}

#[repr(C)]
pub struct Frame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rbp: u64,
    rbx: u64,
    rdx: u64,
    rcx: u64,
    rsi: u64,
    rdi: u64,
    rax: u64,
}

struct Path {
    path: [u8; PATH_MAX],
    length: usize,
}

impl Path {
    const EMPTY: Self = Self {
        path: [0; PATH_MAX],
        length: 0,
    };

    fn path(&self) -> &[u8] {
        &self.path[..self.length]
    }
}

struct Fd {
    position: u64,
    flags: u64,
    socket: Option<network::Socket>,
    file: Option<ramfs::Handle>,
}

impl Fd {
    const EMPTY: Self = Self {
        position: 0,
        flags: 0,
        socket: None,
        file: None,
    };

    fn occupied(&self) -> bool {
        self.file.is_some() || self.socket.is_some()
    }
}

pub struct Process {
    space: Space,
    brk_start: u64,
    brk: u64,
    map_hint: u64,
    fds: [Fd; FDS],
    stdio: u8,
}

impl Process {
    pub fn new(space: Space, brk: u64) -> Self {
        Self {
            space,
            brk_start: brk,
            brk,
            map_hint: MAP_FIRST,
            fds: [Fd::EMPTY; FDS],
            stdio: 7,
        }
    }

    fn descriptor(&mut self, fd: u64) -> Result<&mut Fd, i64> {
        self.fds
            .get_mut(fd.wrapping_sub(3) as usize)
            .filter(|slot| slot.occupied())
            .ok_or(EBADF)
    }

    fn regular_fd(&mut self, fd: u64) -> Result<&mut Fd, i64> {
        self.descriptor(fd).and_then(|slot| {
            if slot.file.is_some() {
                Ok(slot)
            } else {
                Err(EBADF)
            }
        })
    }

    fn vacant(&self) -> Result<usize, i64> {
        self.fds
            .iter()
            .position(|slot| !slot.occupied())
            .ok_or(EMFILE)
    }

    fn socket(&mut self, fd: u64) -> Result<&mut network::Socket, i64> {
        self.descriptor(fd)?.socket.as_mut().ok_or(-88) // ENOTSOCK
    }

    fn standard_open(&self, fd: u64) -> bool {
        fd < 3 && self.stdio & (1 << fd) != 0
    }

    fn close(&mut self, fd: u64) -> i64 {
        if self.standard_open(fd) {
            self.stdio &= !(1 << fd);
            return 0;
        }
        match self.descriptor(fd) {
            Ok(slot) => {
                if let Some(socket) = slot.socket.take() {
                    socket.close();
                }
                *slot = Fd::EMPTY;
                0
            }
            Err(error) => error,
        }
    }

    fn socket_open(&mut self, family: u64, kind: u64, protocol: u64) -> i64 {
        let slot = match self.vacant() {
            Ok(slot) => slot,
            Err(error) => return error,
        };
        match network::open(family, kind, protocol) {
            Ok(socket) => {
                self.fds[slot].socket = Some(socket);
                slot as i64 + 3
            }
            Err(error) => error,
        }
    }

    fn socket_address(&mut self, call: u64, fd: u64, address: u64, length: u64) -> i64 {
        let endpoint = match network::read_address(self.space.0, address, length) {
            Ok(endpoint) => endpoint,
            Err(error) => return error,
        };
        self.socket(fd).map_or_else(
            |error| error,
            |socket| {
                if call == 42 {
                    socket.connect(endpoint)
                } else {
                    socket.bind(endpoint)
                }
            },
        )
    }

    fn socket_name(&mut self, peer: bool, fd: u64, address: u64, length: u64) -> i64 {
        let cr3 = self.space.0;
        match self.socket(fd).and_then(|socket| socket.address(peer)) {
            Ok(endpoint) => network::write_address(cr3, endpoint, address, length),
            Err(error) => error,
        }
    }

    fn socket_accept(&mut self, fd: u64, address: u64, length: u64, flags: u64) -> i64 {
        if flags & !(0x800 | 0x80000) != 0 {
            return EINVAL;
        }
        let slot = match self.vacant() {
            Ok(slot) => slot,
            Err(error) => return error,
        };
        let socket = match self
            .socket(fd)
            .and_then(|socket| socket.accept(flags & 0x800 != 0))
        {
            Ok(socket) => socket,
            Err(error) => return error,
        };
        if address != 0 {
            let written = match socket.address(true) {
                Ok(peer) => network::write_address(self.space.0, peer, address, length),
                Err(error) => error,
            };
            if written != 0 {
                socket.close();
                return written;
            }
        }
        self.fds[slot].socket = Some(socket);
        slot as i64 + 3
    }

    fn socket_write(
        &mut self,
        fd: u64,
        data: u64,
        size: u64,
        flags: u64,
        address: u64,
        length: u64,
    ) -> i64 {
        if flags & !(0x40 | 0x4000) != 0 {
            return -95; // EOPNOTSUPP
        }
        let cr3 = self.space.0;
        let destination = if address == 0 {
            None
        } else {
            match network::read_address(cr3, address, length) {
                Ok(endpoint) => Some(endpoint),
                Err(error) => return error,
            }
        };
        let mut buffer = [0; 1536];
        let socket = match self.socket(fd) {
            Ok(socket) => socket,
            Err(error) => return error,
        };
        if !socket.is_stream() && size > buffer.len() as u64 {
            return -90; // EMSGSIZE
        }
        let count = buffer.len().min(size as usize);
        let bytes = &mut buffer[..count];
        if !memory::copy_in(cr3, data, bytes) {
            return EFAULT;
        }
        socket.send(bytes, destination, flags & 0x40 != 0)
    }

    fn socket_read(
        &mut self,
        fd: u64,
        data: u64,
        size: u64,
        flags: u64,
        address: u64,
        length: u64,
    ) -> i64 {
        if flags & !0x40 != 0 {
            return -95; // EOPNOTSUPP
        }
        let cr3 = self.space.0;
        let mut buffer = [0; 1536];
        let count = buffer.len().min(size as usize);
        let bytes = &mut buffer[..count];
        if !memory::valid(cr3, data, bytes.len(), WRITE) {
            return EFAULT;
        }
        let (received, source) = match self
            .socket(fd)
            .and_then(|socket| socket.receive(bytes, flags & 0x40 != 0))
        {
            Ok(result) => result,
            Err(error) => return error,
        };
        if let Some(source) = source
            && address != 0
        {
            let written = network::write_address(cr3, source, address, length);
            if written != 0 {
                return written;
            }
        }
        out(cr3, data, &bytes[..received], received as i64)
    }

    fn socket_option(
        &mut self,
        set: bool,
        fd: u64,
        level: u64,
        option: u64,
        value: u64,
        length: u64,
    ) -> i64 {
        let cr3 = self.space.0;
        let socket = match self.socket(fd) {
            Ok(socket) => socket,
            Err(error) => return error,
        };
        if level != 1 {
            return -92; // ENOPROTOOPT
        }
        if set {
            if option == 2 {
                if length < 4 {
                    return EINVAL;
                }
                let mut flag = [0; 4];
                if !memory::copy_in(cr3, value, &mut flag) {
                    return EFAULT;
                }
                socket.reuse_address = i32::from_ne_bytes(flag) != 0;
                return 0;
            }
            if !matches!(option, 20 | 21 | 66 | 67) {
                return -92;
            }
            let mut time = [0; 16];
            if length < 16 {
                return EINVAL;
            }
            if !memory::copy_in(cr3, value, &mut time) {
                return EFAULT;
            }
            let seconds = i64::from_ne_bytes(time[..8].try_into().unwrap());
            let micros = i64::from_ne_bytes(time[8..].try_into().unwrap());
            if seconds < 0 || !(0..1_000_000).contains(&micros) {
                return EINVAL;
            }
            let milliseconds = (seconds as u64)
                .saturating_mul(1000)
                .saturating_add((micros as u64).div_ceil(1000));
            if matches!(option, 20 | 66) {
                socket.read_timeout = milliseconds;
            } else {
                socket.write_timeout = milliseconds;
            }
            return 0;
        }
        let mut supplied = [0; 4];
        if !memory::copy_in(cr3, length, &mut supplied) {
            return EFAULT;
        }
        let mut option_bytes = [0; 16];
        let size = match option {
            2 => {
                option_bytes[..4].copy_from_slice(&i32::from(socket.reuse_address).to_ne_bytes());
                4
            }
            3 => {
                let kind = if socket.is_stream() { 1_i32 } else { 2_i32 };
                option_bytes[..4].copy_from_slice(&kind.to_ne_bytes());
                4
            }
            4 => {
                option_bytes[..4].copy_from_slice(&socket.pending_error().to_ne_bytes());
                4
            }
            20 | 21 | 66 | 67 => {
                let ms = if matches!(option, 20 | 66) {
                    socket.read_timeout
                } else {
                    socket.write_timeout
                };
                option_bytes[..8].copy_from_slice(&(ms / 1000).to_ne_bytes());
                option_bytes[8..].copy_from_slice(&((ms % 1000) * 1000).to_ne_bytes());
                16
            }
            _ => return -92,
        };
        let count = (u32::from_ne_bytes(supplied) as usize).min(size);
        if memory::copy_out(cr3, value, &option_bytes[..count])
            && memory::copy_out(cr3, length, &(size as u32).to_ne_bytes())
        {
            0
        } else {
            EFAULT
        }
    }

    fn path(&self, dir: u64, address: u64) -> Result<Path, i64> {
        let mut result = Path::EMPTY;
        let mut input = [0_u8; 1];
        let mut length = 0;
        for offset in 0..PATH_MAX {
            if !memory::copy_in(
                self.space.0,
                address.wrapping_add(offset as u64),
                &mut input,
            ) {
                return Err(EFAULT);
            }
            if input[0] == 0 {
                break;
            }
            if offset == 0 && input[0] != b'/' {
                if dir != AT_FDCWD {
                    return Err(EBADF);
                }
                result.path[0] = b'/';
                length = 1;
            }
            if length == PATH_MAX {
                return Err(ENAMETOOLONG);
            }
            result.path[length] = input[0];
            length += 1;
        }
        if length == 0 {
            return Err(ENOENT);
        }
        if input[0] != 0 {
            return Err(ENAMETOOLONG);
        }

        // Canonicalize separators and dot components so equivalent spellings
        // refer to the same RAMFS node.
        let mut normal = Path::EMPTY;
        normal.path[0] = b'/';
        normal.length = 1;
        for part in result.path[1..length].split(|&byte| byte == b'/') {
            match part {
                b"" | b"." => {}
                b".." => {
                    let previous = normal
                        .path()
                        .iter()
                        .rposition(|&byte| byte == b'/')
                        .unwrap_or(0);
                    normal.length = previous.max(1);
                }
                _ => {
                    if normal.length > 1 {
                        normal.path[normal.length] = b'/';
                        normal.length += 1;
                    }
                    normal.path[normal.length..normal.length + part.len()].copy_from_slice(part);
                    normal.length += part.len();
                }
            }
        }
        Ok(normal)
    }

    fn open(&mut self, dir: u64, address: u64, flags: u64) -> i64 {
        let path = match self.path(dir, address) {
            Ok(path) => path,
            Err(error) => return error,
        };
        let Ok(slot) = self.vacant() else {
            return EMFILE;
        };
        if flags & 3 == 3 {
            return EINVAL;
        }
        match ramfs::stat(path.path()) {
            Ok((directory, _)) => {
                if flags & 0xc0 == 0xc0 {
                    return EEXIST;
                }
                if directory && flags & 3 != 0 {
                    return EISDIR;
                }
                if !directory && flags & 0x10000 != 0 {
                    return ENOTDIR;
                }
                if flags & 0x200 != 0 && flags & 3 != 0 {
                    let error = ramfs::truncate(path.path(), 0);
                    if error != 0 {
                        return fs_error(error);
                    }
                }
            }
            Err(-2) if flags & 0x40 != 0 => {
                let error = ramfs::create(path.path(), false);
                if error != 0 {
                    return fs_error(error);
                }
            }
            Err(error) => return fs_error(error),
        }
        let file = match ramfs::open(path.path()) {
            Ok(file) => Some(file),
            Err(error) => return fs_error(error),
        };
        self.fds[slot] = Fd {
            flags,
            file,
            ..Fd::EMPTY
        };
        slot as i64 + 3
    }

    fn read(&mut self, fd: u64, address: u64, len: u64, offset: Option<u64>) -> i64 {
        if self.socket(fd).is_ok() {
            return if offset.is_none() {
                self.socket_read(fd, address, len, 0, 0, 0)
            } else {
                ESPIPE
            };
        }
        let length = (len as usize).min(IO_MAX);
        let cr3 = self.space.0;
        if !memory::valid(cr3, address, length, WRITE) {
            return EFAULT;
        }
        let mut buffer = [0; 1024];
        if fd < 3 {
            if fd != 0 || !self.standard_open(fd) {
                return EBADF;
            }
            if offset.is_some() {
                return ESPIPE;
            }
            if length == 0 {
                return 0;
            }
            let mut count = 0;
            loop {
                while count < length.min(buffer.len()) {
                    let byte = unsafe { kbd_getc() };
                    if byte < 0 {
                        break;
                    }
                    buffer[count] = byte as u8;
                    count += 1;
                }
                if count != 0 {
                    break;
                }
                arch::wait_for_interrupt();
            }
            memory::copy_out(cr3, address, &buffer[..count]);
            return count as i64;
        }

        let file = match self.regular_fd(fd) {
            Ok(file) => file,
            Err(error) => return error,
        };
        if file.flags & 3 == 1 {
            return EBADF;
        }
        let start = offset.unwrap_or(file.position);
        let mut read = 0;
        while read < length {
            let count = buffer.len().min(length - read);
            let got = file
                .file
                .as_ref()
                .unwrap()
                .read(&mut buffer[..count], start.saturating_add(read as u64));
            if got < 0 {
                return if read == 0 {
                    fs_error(got)
                } else {
                    read as i64
                };
            }
            if got == 0 {
                break;
            }
            memory::copy_out(cr3, address + read as u64, &buffer[..got as usize]);
            read += got as usize;
        }
        if offset.is_none() {
            file.position += read as u64;
        }
        read as i64
    }

    fn write(&mut self, fd: u64, address: u64, len: u64, offset: Option<u64>) -> i64 {
        if self.socket(fd).is_ok() {
            return if offset.is_none() {
                self.socket_write(fd, address, len, 0, 0, 0)
            } else {
                ESPIPE
            };
        }
        let length = (len as usize).min(IO_MAX);
        let cr3 = self.space.0;
        if !memory::valid(cr3, address, length, 0) {
            return EFAULT;
        }
        let mut buffer = [0; 1024];
        if fd < 3 {
            if fd == 0 || !self.standard_open(fd) {
                return EBADF;
            }
            if offset.is_some() {
                return ESPIPE;
            }
            let mut written = 0;
            while written < length {
                let count = buffer.len().min(length - written);
                memory::copy_in(cr3, address + written as u64, &mut buffer[..count]);
                unsafe { bindings::kwrite(buffer.as_ptr().cast(), count as u64, 0x0f) };
                written += count;
            }
            return written as i64;
        }

        let file = match self.regular_fd(fd) {
            Ok(file) => file,
            Err(error) => return error,
        };
        if file.flags & 3 == 0 {
            return EBADF;
        }
        let handle = file.file.as_ref().unwrap();
        let start = if file.flags & 0x400 != 0 {
            handle.stat().1 as u64
        } else {
            offset.unwrap_or(file.position)
        };

        let mut written = 0;
        while written < length {
            let count = buffer.len().min(length - written);
            memory::copy_in(cr3, address + written as u64, &mut buffer[..count]);
            let put = handle.write(&buffer[..count], start.saturating_add(written as u64));
            if put < 0 {
                if written == 0 {
                    return fs_error(put);
                }
                break;
            }
            written += put as usize;
            if put < count as i64 {
                break;
            }
        }
        if offset.is_none() {
            file.position = start + written as u64;
        }
        written as i64
    }

    fn seek(&mut self, fd: u64, offset: i64, whence: u64) -> i64 {
        if self.standard_open(fd) {
            return ESPIPE;
        }
        let file = match self.descriptor(fd) {
            Ok(file) => file,
            Err(error) => return error,
        };
        if file.socket.is_some() {
            return ESPIPE;
        }
        let base = match whence {
            0 => 0,
            1 => file.position,
            2 => file.file.as_ref().unwrap().stat().1 as u64,
            _ => return EINVAL,
        };
        let Some(position) = base
            .checked_add_signed(offset)
            .filter(|&value| value <= i64::MAX as u64)
        else {
            return EINVAL;
        };
        file.position = position;
        position as i64
    }

    fn stat(&mut self, dir: u64, source: u64, out: u64, flags: u64) -> i64 {
        let result = if flags & 0x1000 != 0 {
            let mut byte = [0];
            if !memory::copy_in(self.space.0, source, &mut byte) {
                return EFAULT;
            }
            if byte[0] == 0 {
                return self.fstat(dir, out);
            }
            self.path(dir, source)
        } else {
            self.path(dir, source)
        };
        match result {
            Ok(path) => self.stat_path(path.path(), out),
            Err(error) => error,
        }
    }

    fn stat_path(&self, path: &[u8], out: u64) -> i64 {
        match ramfs::stat_with_inode(path) {
            Ok((directory, size, inode)) => stat_to_user(
                self.space.0,
                out,
                if directory { 0o40777 } else { 0o100666 },
                size,
                inode,
            ),
            Err(error) => fs_error(error),
        }
    }

    fn fstat(&mut self, fd: u64, address: u64) -> i64 {
        let cr3 = self.space.0;
        if self.standard_open(fd) {
            return stat_to_user(cr3, address, 0o20666, 0, 2);
        }
        match self.descriptor(fd) {
            Ok(slot) => {
                if slot.socket.is_some() {
                    return stat_to_user(cr3, address, 0o140666, 0, fd);
                }
                let (directory, size, inode) = slot.file.as_ref().unwrap().stat();
                stat_to_user(
                    cr3,
                    address,
                    if directory { 0o40777 } else { 0o100666 },
                    size,
                    inode,
                )
            }
            Err(error) => error,
        }
    }

    fn map(&mut self, address: u64, length: u64, prot: u64, flags: u64) -> i64 {
        let Some(length) =
            memory::page_ceil(length).filter(|&size| size != 0 && size <= MAX_MAPPING)
        else {
            return EINVAL;
        };
        let Some(perms) = page_permissions(prot) else {
            return EACCES;
        };
        if flags & 0x20 == 0 || flags & 3 != 2 {
            return ENOSYS;
        }

        let fixed = flags & (0x10 | 0x100000) != 0;
        let mut start = if address != 0 {
            memory::page_floor(address)
        } else {
            self.map_hint
        };
        let cr3 = self.space.0;
        if fixed {
            if start != address || !map_range(start, length) {
                return EINVAL;
            }
        } else if !map_range(start, length) {
            start = MAP_FIRST;
        }
        loop {
            let occupied = (start..start + length)
                .step_by(PAGE as usize)
                .find(|&page| memory::entry(cr3, page) & PRESENT != 0);
            match occupied {
                None => break,
                Some(_) if fixed => return EEXIST,
                Some(page) => {
                    start = page + PAGE;
                    if !map_range(start, length) {
                        return ENOMEM;
                    }
                }
            }
        }

        if !map_pages(cr3, start, start + length, perms) {
            return ENOMEM;
        }
        if !fixed {
            self.map_hint = start + length;
        }
        start as i64
    }

    fn unmap(&mut self, address: u64, length: u64) -> i64 {
        let Some(length) =
            memory::page_ceil(length).filter(|&size| size != 0 && size <= MAX_MAPPING)
        else {
            return EINVAL;
        };
        if address % PAGE != 0 || !map_range(address, length) {
            return EINVAL;
        }
        for page in (address..address + length).step_by(PAGE as usize) {
            memory::unmap(self.space.0, page);
        }
        self.map_hint = self.map_hint.min(address);
        0
    }

    fn protect(&mut self, address: u64, length: u64, prot: u64) -> i64 {
        let Some(length) = memory::page_ceil(length).filter(|&size| size <= MAX_MAPPING) else {
            return EINVAL;
        };
        let Some(end) = address.checked_add(length) else {
            return EINVAL;
        };
        let Some(perms) = page_permissions(prot) else {
            return EACCES;
        };
        if address % PAGE != 0
            || !(map_range(address, length)
                || memory::image_range(address, end)
                || (address >= memory::STACK_TOP - memory::STACK_SIZE && end <= memory::STACK_TOP))
        {
            return EINVAL;
        }
        let pages = (address..end).step_by(PAGE as usize);
        if pages.clone().any(|page| !memory::owned(self.space.0, page)) {
            return ENOMEM;
        }
        for page in pages {
            memory::protect(self.space.0, page, perms);
        }
        0
    }

    fn brk(&mut self, address: u64) -> i64 {
        let old = self.brk as i64;
        if address < self.brk_start || address - self.brk_start > MAX_MAPPING {
            return old;
        }
        let Some(end) = memory::page_ceil(address) else {
            return old;
        };
        if end != self.brk_start && !memory::image_range(self.brk_start, end) {
            return old;
        }
        let from = memory::page_ceil(self.brk).unwrap();
        if end > from {
            if (from..end)
                .step_by(PAGE as usize)
                .any(|page| memory::owned(self.space.0, page))
                || !map_pages(self.space.0, from, end, WRITE | NX)
            {
                return old;
            }
        } else {
            for address in (end..from).step_by(PAGE as usize) {
                memory::unmap(self.space.0, address);
            }
        }
        self.brk = address;
        address as i64
    }

    fn iov(&mut self, write: bool, fd: u64, vectors: u64, count: u64) -> i64 {
        if count > 1024 {
            return EINVAL;
        }
        let mut total = 0;
        for i in 0..count {
            let Some(address) = vectors.checked_add(i * 16) else {
                return EFAULT;
            };
            let mut iov = [0; 16];
            if !memory::copy_in(self.space.0, address, &mut iov) {
                return if total == 0 { EFAULT } else { total };
            }
            let base = u64::from_ne_bytes(iov[..8].try_into().unwrap());
            let length = u64::from_ne_bytes(iov[8..].try_into().unwrap()).min(IO_MAX as u64);
            let result = if write {
                self.write(fd, base, length, None)
            } else {
                self.read(fd, base, length, None)
            };
            if result < 0 {
                return if total == 0 { result } else { total };
            }
            total += result;
            if result as u64 != length || (!write && fd == 0 && result > 0) {
                break;
            }
        }
        total
    }

    fn call(&mut self, frame: &Frame, thread: *mut Thread) -> i64 {
        let cr3 = self.space.0;
        let (a, b, c, d, e, f) = (
            frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8, frame.r9,
        );
        match frame.rax {
            0 => self.read(a, b, c, None),
            1 => self.write(a, b, c, None),
            2 => self.open(AT_FDCWD, a, b),
            3 => self.close(a),
            4 | 6 => self.stat(AT_FDCWD, a, b, 0),
            5 => self.fstat(a, b),
            7 => self.poll(a, b, c as i32),
            8 => self.seek(a, b as i64, c),
            9 => self.map(a, b, c, d),
            10 => self.protect(a, b, c),
            11 => self.unmap(a, b),
            12 => self.brk(a),
            13 | 14 | 28 | 131 => 0, // signal bookkeeping and memory advice
            16 => {
                if self.standard_open(a) && a == 1 {
                    super::graphics::ioctl(unsafe { (*thread).tid }, cr3, b, c).unwrap_or(ENOTTY)
                } else if !self.standard_open(a) && self.descriptor(a).is_err() {
                    EBADF
                } else {
                    ENOTTY
                }
            }
            17 => self.read(a, b, c, Some(d)),
            18 => self.write(a, b, c, Some(d)),
            19 => self.iov(false, a, b, c),
            20 => self.iov(true, a, b, c),
            21 => self.access(AT_FDCWD, a),
            24 => {
                arch::wait_for_interrupt();
                0
            }
            35 => sleep(cr3, a),
            39 | 186 | 218 => unsafe { (*thread).tid as i64 },
            41 => self.socket_open(a, b, c),
            42 | 49 => self.socket_address(frame.rax, a, b, c),
            43 => self.socket_accept(a, b, c, 0),
            44 => self.socket_write(a, b, c, d, e, f),
            45 => self.socket_read(a, b, c, d, e, f),
            48 => self
                .socket(a)
                .map_or_else(|error| error, |socket| socket.shutdown(b)),
            50 => self
                .socket(a)
                .map_or_else(|error| error, |socket| socket.listen()),
            51 | 52 => self.socket_name(frame.rax == 52, a, b, c),
            54 | 55 => self.socket_option(frame.rax == 54, a, b, c, d, e),
            60 | 231 => exit((a & 255) as i64),
            63 => uname(cr3, a),
            72 => self.fcntl(a, b, c),
            74 | 75 => self.regular_fd(a).map_or_else(|error| error, |_| 0),
            77 => match self.regular_fd(a) {
                Ok(file) if file.flags & 3 != 0 => {
                    fs_result(file.file.as_ref().unwrap().truncate(b))
                }
                Ok(_) => EINVAL,
                Err(error) => error,
            },
            79 if b >= 2 => out(cr3, a, b"/\0", 2),
            79 => -34, // ERANGE
            82 => self.rename(AT_FDCWD, a, AT_FDCWD, b),
            83 => self.make_dir(AT_FDCWD, a),
            84 => self.remove_dir(AT_FDCWD, a),
            87 => self.unlink(AT_FDCWD, a),
            89 => ENOENT,
            96 => timestamp(cr3, a, true),
            97 => getrlimit(cr3, a, b),
            102 | 104 | 107 | 108 => 1000,
            110 => 1,
            158 => arch_prctl(cr3, unsafe { (*thread).context }, a, b),
            202 => futex(cr3, a, b, c),
            204 => {
                if b == 0 {
                    EINVAL
                } else {
                    out(cr3, c, &[1], 1)
                }
            }
            217 => self.getdents(a, b, c),
            228 => match a {
                0 | 1 | 4 | 5 | 6 | 7 => timestamp(cr3, b, false),
                _ => EINVAL,
            },
            230 if b == 0 => sleep(cr3, c),
            257 => self.open(a, b, c),
            258 => self.make_dir(a, b),
            262 => self.stat(a, b, c, d),
            263 if c == 0 => self.unlink(a, b),
            263 if c == 0x200 => self.remove_dir(a, b),
            264 => self.rename(a, b, c, d),
            267 => ENOENT,
            269 | 439 => self.access(a, b),
            273 => ENOSYS, // no robust futex lists
            288 => self.socket_accept(a, b, c, d),
            302 if a == 0 && c == 0 => getrlimit(cr3, b, d),
            318 => getrandom(cr3, a, b, c),
            334 => ENOSYS, // restartable sequences
            _ => ENOSYS,
        }
    }

    fn unlink(&self, dir: u64, address: u64) -> i64 {
        match self.path(dir, address) {
            Ok(path) => fs_result(ramfs::unlink(path.path())),
            Err(error) => error,
        }
    }

    fn rename(&self, old_dir: u64, old: u64, new_dir: u64, new: u64) -> i64 {
        let old = match self.path(old_dir, old) {
            Ok(path) => path,
            Err(error) => return error,
        };
        match self.path(new_dir, new) {
            Ok(path) => fs_result(ramfs::rename(old.path(), path.path())),
            Err(error) => error,
        }
    }

    fn getdents(&mut self, fd: u64, address: u64, length: u64) -> i64 {
        let cr3 = self.space.0;
        let slot = match self.regular_fd(fd) {
            Ok(slot) => slot,
            Err(error) => return error,
        };
        let directory = slot.file.as_ref().unwrap();
        if !directory.stat().0 {
            return ENOTDIR;
        }
        let mut written = 0;
        while let Some(entry) = directory.directory_entry(slot.position as usize) {
            let count = (20 + entry.length).next_multiple_of(8);
            if written + count as u64 > length {
                return if written == 0 { EINVAL } else { written as i64 };
            }
            let Some(out) = address.checked_add(written) else {
                return EFAULT;
            };
            let mut record = [0; 88];
            record[..8].copy_from_slice(&entry.inode.to_ne_bytes());
            record[8..16].copy_from_slice(&(slot.position + 1).to_ne_bytes());
            record[16..18].copy_from_slice(&(count as u16).to_ne_bytes());
            record[18] = if entry.directory { 4 } else { 8 };
            record[19..19 + entry.length].copy_from_slice(&entry.name[..entry.length]);
            if !memory::copy_out(cr3, out, &record[..count]) {
                return if written == 0 { EFAULT } else { written as i64 };
            }
            written += count as u64;
            slot.position += 1;
        }
        written as i64
    }

    fn make_dir(&self, dir: u64, address: u64) -> i64 {
        match self.path(dir, address) {
            Ok(path) => fs_result(ramfs::create(path.path(), true)),
            Err(error) => error,
        }
    }

    fn remove_dir(&self, dir: u64, address: u64) -> i64 {
        match self.path(dir, address) {
            Ok(path) => fs_result(ramfs::remove(path.path(), true)),
            Err(error) => error,
        }
    }

    fn access(&self, dir: u64, address: u64) -> i64 {
        match self.path(dir, address) {
            Ok(path) => match ramfs::stat(path.path()) {
                Ok(_) => 0,
                Err(error) => fs_error(error),
            },
            Err(error) => error,
        }
    }

    fn poll(&mut self, address: u64, count: u64, timeout: i32) -> i64 {
        let deadline = ticks().saturating_add(timeout.max(0) as u64);
        loop {
            let result = self.poll_once(address, count);
            if result != 0 || (timeout >= 0 && ticks() >= deadline) {
                return result;
            }
            arch::wait_for_interrupt();
        }
    }

    fn poll_once(&mut self, address: u64, count: u64) -> i64 {
        if count > 1024 || !memory::valid(self.space.0, address, count as usize * 8, WRITE) {
            return EFAULT;
        }
        let mut ready = 0;
        for i in 0..count {
            let mut record = [0; 8];
            let position = address + i * 8;
            memory::copy_in(self.space.0, position, &mut record);
            let fd = i32::from_ne_bytes(record[..4].try_into().unwrap());
            let events = u16::from_ne_bytes(record[4..6].try_into().unwrap());
            let result: u16 = if fd < 0 {
                0
            } else if self.standard_open(fd as u64) {
                if fd == 0 { 0 } else { events & 4 }
            } else if let Ok(entry) = self.descriptor(fd as u64) {
                entry
                    .socket
                    .map_or(events & 5, |socket| socket.events(events))
            } else {
                0x20 // POLLNVAL
            };
            memory::copy_out(self.space.0, position + 6, &result.to_ne_bytes());
            ready += i64::from(result != 0);
        }
        ready
    }

    fn fcntl(&mut self, fd: u64, operation: u64, argument: u64) -> i64 {
        if self.standard_open(fd) {
            return match operation {
                1 | 2 => 0,
                3 => i64::from(fd != 0),
                _ => ENOSYS,
            };
        }
        match self.descriptor(fd) {
            Ok(file) if file.socket.is_some() => {
                let socket = file.socket.as_mut().unwrap();
                match operation {
                    1 | 2 => 0,
                    3 => {
                        if socket.nonblocking {
                            0x802
                        } else {
                            2
                        }
                    }
                    4 => {
                        socket.nonblocking = argument & 0x800 != 0;
                        0
                    }
                    _ => ENOSYS,
                }
            }
            Ok(file) => match operation {
                1 | 2 => 0,
                3 => file.flags as i64 & 0xc03,
                _ => ENOSYS,
            },
            Err(error) => error,
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        for slot in &mut self.fds {
            if let Some(socket) = slot.socket.take() {
                socket.close();
            }
        }
    }
}

fn map_range(address: u64, length: u64) -> bool {
    address >= MAP_FIRST
        && address
            .checked_add(length)
            .is_some_and(|end| end <= MAP_END)
}

fn page_permissions(prot: u64) -> Option<u64> {
    if prot == 0 {
        return Some(NX | NO_ACCESS);
    }
    if prot & !7 != 0 || prot & 6 == 6 {
        return None;
    }
    Some((if prot & 2 != 0 { WRITE } else { 0 }) | if prot & 4 != 0 { 0 } else { NX })
}

fn map_pages(cr3: u64, first: u64, end: u64, perms: u64) -> bool {
    for page in (first..end).step_by(PAGE as usize) {
        if !memory::map_new(cr3, page, perms) {
            for allocated in (first..page).step_by(PAGE as usize) {
                memory::unmap(cr3, allocated);
            }
            return false;
        }
    }
    true
}

fn fs_result(result: i64) -> i64 {
    if result < 0 { fs_error(result) } else { result }
}

fn fs_error(error: i64) -> i64 {
    match error {
        -2 => ENOENT,
        -3 => EEXIST,
        -4 => ENOTDIR,
        -5 => EISDIR,
        -6 => ENOSPC,
        -8 => -39, // ENOTEMPTY
        _ => EINVAL,
    }
}

fn out(cr3: u64, address: u64, bytes: &[u8], result: i64) -> i64 {
    if memory::copy_out(cr3, address, bytes) {
        result
    } else {
        EFAULT
    }
}

fn stat_to_user(cr3: u64, address: u64, mode: u32, size: usize, inode: u64) -> i64 {
    // x86-64 struct stat: dev, ino, nlink, mode, uid, gid, padding,
    // rdev, size, block size, block count, timestamps and reserved words.
    let mut bytes = [0; 144];
    for (offset, value) in [
        (0, 1),
        (8, inode),
        (16, 1),
        (48, size as u64),
        (56, PAGE),
        (64, size.div_ceil(512) as u64),
    ] {
        bytes[offset..offset + 8].copy_from_slice(&value.to_ne_bytes());
    }
    for (offset, value) in [(24, mode), (28, 1000), (32, 1000)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    out(cr3, address, &bytes, 0)
}

fn arch_prctl(cr3: u64, context: *mut Context, operation: u64, address: u64) -> i64 {
    match operation {
        0x1002 if address < 0x8000_0000_0000 => {
            unsafe { (*context).fs_base = address };
            arch::set_fs_base(address);
            0
        }
        0x1002 => EPERM,
        0x1003 => out(
            cr3,
            address,
            &unsafe { (*context).fs_base }.to_ne_bytes(),
            0,
        ),
        _ => EINVAL,
    }
}

pub(super) fn ticks() -> u64 {
    unsafe { ptr::addr_of!(lapic_ticks_ms).read_volatile() }
}

fn timestamp(cr3: u64, address: u64, micros: bool) -> i64 {
    if micros && address == 0 {
        return 0;
    }
    let ticks = ticks();
    let mut buf = [0; 16];
    buf[..8].copy_from_slice(&(ticks / 1000).to_ne_bytes());
    let units = if micros { 1000 } else { 1_000_000 };
    buf[8..].copy_from_slice(&((ticks % 1000) * units).to_ne_bytes());
    out(cr3, address, &buf, 0)
}

fn sleep(cr3: u64, address: u64) -> i64 {
    let mut buf = [0; 16];
    if !memory::copy_in(cr3, address, &mut buf) {
        return EFAULT;
    }
    let sec = i64::from_ne_bytes(buf[..8].try_into().unwrap());
    let nsec = u64::from_ne_bytes(buf[8..].try_into().unwrap());
    if !(0..=86_400).contains(&sec) || nsec >= 1_000_000_000 {
        return EINVAL;
    }
    let end = ticks().saturating_add(sec as u64 * 1000 + nsec.div_ceil(1_000_000));
    while ticks() < end {
        arch::wait_for_interrupt();
    }
    0
}

fn getrlimit(cr3: u64, resource: u64, address: u64) -> i64 {
    if address == 0 {
        return 0;
    }
    let value = match resource {
        3 => memory::STACK_SIZE,
        7 => FDS as u64 + 3,
        0..=15 => u64::MAX,
        _ => return EINVAL,
    };
    let mut buf = [0; 16];
    buf[..8].copy_from_slice(&value.to_ne_bytes());
    buf[8..].copy_from_slice(&value.to_ne_bytes());
    out(cr3, address, &buf, 0)
}

fn futex(cr3: u64, address: u64, operation: u64, value: u64) -> i64 {
    let mut bytes = [0; 4];
    if address % 4 != 0 || !memory::copy_in(cr3, address, &mut bytes) {
        return EFAULT;
    }
    match operation & 0x7f {
        1 | 10 => 0,
        0 | 9 if u32::from_ne_bytes(bytes) != value as u32 => EAGAIN,
        _ => ENOSYS,
    }
}

fn getrandom(cr3: u64, address: u64, length: u64, flags: u64) -> i64 {
    let length = (length as usize).min(IO_MAX);
    if flags & !3 != 0 {
        return EINVAL;
    }
    if !memory::valid(cr3, address, length, WRITE) {
        return EFAULT;
    }
    let mut filled = 0;
    while filled < length {
        let Some(value) = arch::random() else {
            return if filled == 0 { ENOSYS } else { filled as i64 };
        };
        let count = (length - filled).min(8);
        memory::copy_out(cr3, address + filled as u64, &value.to_ne_bytes()[..count]);
        filled += count;
    }
    filled as i64
}

fn uname(cr3: u64, address: u64) -> i64 {
    let mut buf = [0; 65 * 6];
    for (i, field) in [
        b"Lumen".as_slice(),
        b"lumen",
        b"0.1.0",
        b"static musl ABI",
        b"x86_64",
        b"local",
    ]
    .iter()
    .enumerate()
    {
        buf[i * 65..i * 65 + field.len()].copy_from_slice(field);
    }
    out(cr3, address, &buf, 0)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn lumen_elf_syscall(frame: *mut Frame) {
    let frame = unsafe { &mut *frame };
    let thread = unsafe { current_thread };
    if thread.is_null() || unsafe { (*thread).process }.is_null() {
        frame.rax = ENOSYS as u64;
        return;
    }
    frame.rax = unsafe { &mut *(*thread).process.cast::<Process>() }.call(frame, thread) as u64;
}
