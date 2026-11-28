//! Lumen native application syscalls for isolated ELF processes.
use super::{
    accounts,
    process::{self, Identity},
};
use crate::abi;
use core::ptr;

use super::{Context, Thread, arch, current_thread, exit, memory, network};
use crate::{bindings, ramfs};
use memory::{MAP_END, MAP_FIRST, NO_ACCESS, NX, PAGE, PRESENT, Space, WRITE};

const EPERM: i64 = -1;
const ENOENT: i64 = -2;
const EBADF: i64 = -9;
const ENOMEM: i64 = -12;
const EACCES: i64 = -13;
const EFAULT: i64 = -14;
const EEXIST: i64 = -17;
const ENOTDIR: i64 = -20;
const EISDIR: i64 = -21;
const EINVAL: i64 = -22;
const EMFILE: i64 = -24;
const ENOSPC: i64 = -28;
const ESPIPE: i64 = -29;
const ENAMETOOLONG: i64 = -36;
const ENOSYS: i64 = -38;
const AT_FDCWD: u64 = abi::CURRENT_DIRECTORY;
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
    owner: u32,
}

impl Fd {
    const EMPTY: Self = Self {
        position: 0,
        flags: 0,
        socket: None,
        file: None,
        owner: 0,
    };

    fn occupied(&self) -> bool {
        self.file.is_some() || self.socket.is_some()
    }
}

pub struct Process {
    space: Space,
    identity: Identity,
    brk_start: u64,
    brk: u64,
    map_hint: u64,
    fds: [Fd; FDS],
    stdio: u8,
}

impl Process {
    pub fn new(space: Space, brk: u64, identity: Identity) -> Self {
        Self {
            space,
            identity,
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

    fn socket_open(&mut self, kind: u64, flags: u64) -> i64 {
        let slot = match self.vacant() {
            Ok(slot) => slot,
            Err(error) => return error,
        };
        match network::open(kind, flags) {
            Ok(socket) => {
                self.fds[slot].socket = Some(socket);
                slot as i64 + 3
            }
            Err(error) => error,
        }
    }

    fn socket_address(&mut self, connect: bool, fd: u64, address: u64) -> i64 {
        let endpoint = match network::read_address(self.space.0, address) {
            Ok(endpoint) => endpoint,
            Err(error) => return error,
        };
        self.socket(fd).map_or_else(
            |error| error,
            |socket| {
                if connect {
                    socket.connect(endpoint)
                } else {
                    socket.bind(endpoint)
                }
            },
        )
    }

    fn socket_name(&mut self, peer: bool, fd: u64, address: u64) -> i64 {
        let cr3 = self.space.0;
        match self.socket(fd).and_then(|socket| socket.address(peer)) {
            Ok(endpoint) => network::write_address(cr3, endpoint, address),
            Err(error) => error,
        }
    }

    fn socket_accept(&mut self, fd: u64, address: u64, flags: u64) -> i64 {
        if flags & !abi::NONBLOCK != 0 {
            return EINVAL;
        }
        let slot = match self.vacant() {
            Ok(slot) => slot,
            Err(error) => return error,
        };
        let socket = match self
            .socket(fd)
            .and_then(|socket| socket.accept(flags & abi::NONBLOCK != 0))
        {
            Ok(socket) => socket,
            Err(error) => return error,
        };
        if address != 0 {
            let written = match socket.address(true) {
                Ok(peer) => network::write_address(self.space.0, peer, address),
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

    fn socket_write(&mut self, fd: u64, data: u64, size: u64, flags: u64, address: u64) -> i64 {
        if flags & !abi::NONBLOCK != 0 {
            return -95; // EOPNOTSUPP
        }
        let cr3 = self.space.0;
        let destination = if address == 0 {
            None
        } else {
            match network::read_address(cr3, address) {
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
        socket.send(bytes, destination, flags & abi::NONBLOCK != 0)
    }

    fn socket_read(&mut self, fd: u64, data: u64, size: u64, flags: u64, address: u64) -> i64 {
        if flags & !abi::NONBLOCK != 0 {
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
            .and_then(|socket| socket.receive(bytes, flags & abi::NONBLOCK != 0))
        {
            Ok(result) => result,
            Err(error) => return error,
        };
        if let Some(source) = source
            && address != 0
        {
            let written = network::write_address(cr3, source, address);
            if written != 0 {
                return written;
            }
        }
        out(cr3, data, &bytes[..received], received as i64)
    }

    fn socket_option(&mut self, fd: u64, option: u64, set: bool, value: u64) -> i64 {
        let socket = match self.socket(fd) {
            Ok(socket) => socket,
            Err(error) => return error,
        };
        match (option, set) {
            (abi::SOCKET_KIND, false) => {
                if socket.is_stream() {
                    1
                } else {
                    2
                }
            }
            (abi::SOCKET_ERROR, false) => i64::from(socket.pending_error()),
            (abi::SOCKET_REUSE, false) => i64::from(socket.reuse_address),
            (abi::SOCKET_READ_TIMEOUT, false) => socket.read_timeout as i64,
            (abi::SOCKET_WRITE_TIMEOUT, false) => socket.write_timeout as i64,
            (abi::SOCKET_REUSE, true) => {
                socket.reuse_address = value != 0;
                0
            }
            (abi::SOCKET_READ_TIMEOUT, true) => {
                socket.read_timeout = value.min(i64::MAX as u64);
                0
            }
            (abi::SOCKET_WRITE_TIMEOUT, true) => {
                socket.write_timeout = value.min(i64::MAX as u64);
                0
            }
            _ => -92,
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
                let mut home = [0; 32];
                length = accounts::home(self.identity.account, &mut home);
                result.path[..length].copy_from_slice(&home[..length]);
                if length != 1 {
                    result.path[length] = b'/';
                    length += 1;
                }
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

    // Accounts have a private home tree. System executables and the bundled
    // game data are shared read-only. Boot services retain their own ABI.
    fn can_access(&self, path: &[u8], write: bool) -> bool {
        if self.identity.account.uid == 0 {
            return true;
        }
        let mut home = [0; 32];
        let length = accounts::home(self.identity.account, &mut home);
        let home = &home[..length];
        (!write && path == home)
            || (path.starts_with(home) && path.get(length) == Some(&b'/'))
            || (!write
                && (path == b"/bin"
                    || path.starts_with(b"/bin/")
                    || matches!(path, b"/freedoom1.wad" | b"/readme.txt")))
    }

    fn open(&mut self, dir: u64, address: u64, flags: u64) -> i64 {
        let path = match self.path(dir, address) {
            Ok(path) => path,
            Err(error) => return error,
        };
        if flags & (abi::OPEN_READ | abi::OPEN_WRITE) == 0 || flags & !255 != 0 {
            return EINVAL;
        }
        let write = flags & (abi::OPEN_WRITE | abi::OPEN_CREATE | abi::OPEN_TRUNCATE) != 0;
        if !self.can_access(path.path(), write) {
            return EACCES;
        }
        let Ok(slot) = self.vacant() else {
            return EMFILE;
        };
        match ramfs::stat(path.path()) {
            Ok((directory, _)) => {
                if flags & (abi::OPEN_CREATE | abi::OPEN_EXCLUSIVE)
                    == abi::OPEN_CREATE | abi::OPEN_EXCLUSIVE
                {
                    return EEXIST;
                }
                if directory && flags & abi::OPEN_WRITE != 0 {
                    return EISDIR;
                }
                if !directory && flags & abi::OPEN_DIRECTORY != 0 {
                    return ENOTDIR;
                }
                if flags & abi::OPEN_TRUNCATE != 0 && flags & abi::OPEN_WRITE != 0 {
                    let error = ramfs::truncate(path.path(), 0);
                    if error != 0 {
                        return fs_error(error);
                    }
                }
            }
            Err(-2) if flags & abi::OPEN_CREATE != 0 && flags & abi::OPEN_DIRECTORY == 0 => {
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
            owner: accounts::owner(path.path()),
            ..Fd::EMPTY
        };
        slot as i64 + 3
    }

    fn read(&mut self, fd: u64, address: u64, len: u64, offset: Option<u64>) -> i64 {
        if self.socket(fd).is_ok() {
            return if offset.is_none() {
                self.socket_read(fd, address, len, 0, 0)
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
            if length == 0 || self.identity.captured {
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
        if file.flags & abi::OPEN_READ == 0 {
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
                self.socket_write(fd, address, len, 0, 0)
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
            if self.identity.captured && length != 0 {
                let tid = unsafe { (*current_thread).tid };
                let count = length.min(abi::IPC_MAX);
                let result = process::send(
                    tid,
                    self.identity,
                    cr3,
                    self.identity.parent,
                    address,
                    count as u64,
                );
                return if result == 1 { count as i64 } else { result };
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
        if file.flags & abi::OPEN_WRITE == 0 {
            return EBADF;
        }
        let handle = file.file.as_ref().unwrap();
        let start = if file.flags & abi::OPEN_APPEND != 0 {
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

    fn info_path(&self, dir: u64, source: u64, address: u64) -> i64 {
        let path = match self.path(dir, source) {
            Ok(path) => path,
            Err(error) => return error,
        };
        if !self.can_access(path.path(), false) {
            return EACCES;
        }
        match ramfs::stat_with_inode(path.path()) {
            Ok((directory, size, inode)) => file_info(
                self.space.0,
                address,
                inode,
                size,
                if directory {
                    abi::FILE_DIRECTORY
                } else {
                    abi::FILE_REGULAR
                },
                accounts::owner(path.path()),
            ),
            Err(error) => fs_error(error),
        }
    }

    fn info_handle(&mut self, fd: u64, address: u64) -> i64 {
        let cr3 = self.space.0;
        if self.standard_open(fd) {
            return file_info(cr3, address, 2, 0, abi::FILE_CONSOLE, 0);
        }
        match self.descriptor(fd) {
            Ok(slot) if slot.socket.is_some() => {
                file_info(cr3, address, fd, 0, abi::FILE_SOCKET, 0)
            }
            Ok(slot) => {
                let (directory, size, inode) = slot.file.as_ref().unwrap().stat();
                file_info(
                    cr3,
                    address,
                    inode,
                    size,
                    if directory {
                        abi::FILE_DIRECTORY
                    } else {
                        abi::FILE_REGULAR
                    },
                    slot.owner,
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
        if flags & !abi::MAP_FIXED != 0 {
            return EINVAL;
        }

        let fixed = flags & abi::MAP_FIXED != 0;
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

    fn call(&mut self, frame: &Frame, thread: *mut Thread) -> i64 {
        let cr3 = self.space.0;
        let tid = unsafe { (*thread).tid };
        let (a, b, c, d, e) = (frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8);
        match frame.rax {
            abi::EXIT => exit((a & 255) as i64),
            abi::WRITE => self.write(a, b, c, None),
            abi::READ => self.read(a, b, c, None),
            abi::YIELD => sleep(0),
            abi::SELF => tid as i64,
            abi::UPTIME => ticks() as i64,
            abi::SEND => process::send(tid, self.identity, cr3, a, b, c),
            abi::RECEIVE => process::receive(cr3, a, b, c),
            abi::SPAWN => match self.path(abi::CURRENT_DIRECTORY, a) {
                Ok(path) if self.can_access(path.path(), false) => {
                    process::spawn_from(tid, self.identity, cr3, path.path(), b, c, d, e)
                }
                Ok(_) => EACCES,
                Err(error) => error,
            },
            abi::WAIT => process::wait(tid, a, b != 0),
            abi::IDENTITY => match a {
                0 => i64::from(self.identity.account.uid),
                1 => i64::from(self.identity.account.admin),
                2 => self.identity.parent as i64,
                _ => EINVAL,
            },
            abi::ACCOUNT_REGISTER if self.identity.account.uid == 0 => accounts::register(cr3, a),
            abi::ACCOUNT_REGISTER => EPERM,
            abi::ACCOUNT_DESCRIBE => {
                if self.identity.account.uid != 0 && a != u64::from(self.identity.account.uid) {
                    return EPERM;
                }
                match u32::try_from(a).ok().and_then(accounts::lookup) {
                    Some(account) => accounts::describe(account, cr3, b),
                    None => ENOENT,
                }
            }
            abi::OPEN => self.open(c, a, b),
            abi::CLOSE => self.close(a),
            abi::READ_AT => self.read(a, b, c, Some(d)),
            abi::WRITE_AT => self.write(a, b, c, Some(d)),
            abi::INFO_PATH => self.info_path(c, a, b),
            abi::INFO_HANDLE => self.info_handle(a, b),
            abi::SEEK => self.seek(a, b as i64, c),
            abi::RESIZE_FILE => match self.regular_fd(a) {
                Ok(file) if file.flags & abi::OPEN_WRITE != 0 => {
                    fs_result(file.file.as_ref().unwrap().truncate(b))
                }
                Ok(_) => EINVAL,
                Err(error) => error,
            },
            abi::SYNC_FILE => self.regular_fd(a).map_or_else(|error| error, |_| 0),
            abi::CREATE_DIRECTORY => self.make_dir(b, a),
            abi::REMOVE_PATH if c == 0 => self.unlink(b, a),
            abi::REMOVE_PATH if c == 1 => self.remove_dir(b, a),
            abi::MOVE_PATH => self.rename(b, a, d, c),
            abi::READ_DIRECTORY => self.read_directory(a, b),
            abi::WORKING_DIRECTORY => {
                let mut path = [0; 32];
                let length = accounts::home(self.identity.account, &mut path) + 1;
                if b < length as u64 {
                    -34
                } else {
                    out(cr3, a, &path[..length], length as i64)
                }
            }
            abi::HANDLE_FLAGS => self.handle_flags(a, b, c),
            abi::POLL => self.poll(a, b, c as i32),
            abi::MAP_MEMORY => self.map(a, b, c, d),
            abi::UNMAP_MEMORY => self.unmap(a, b),
            abi::PROTECT_MEMORY => self.protect(a, b, c),
            abi::BREAK => self.brk(a),
            abi::SET_TLS => set_tls(unsafe { (*thread).context }, a),
            abi::GET_TLS => out(
                cr3,
                a,
                &unsafe { (*(*thread).context).fs_base }.to_ne_bytes(),
                0,
            ),
            abi::RANDOM => getrandom(cr3, a, b),
            abi::SLEEP => sleep(a),
            abi::OPEN_SOCKET => self.socket_open(a, b),
            abi::CONNECT_SOCKET | abi::BIND_SOCKET => {
                self.socket_address(frame.rax == abi::CONNECT_SOCKET, a, b)
            }
            abi::LISTEN_SOCKET => self
                .socket(a)
                .map_or_else(|error| error, |socket| socket.listen()),
            abi::ACCEPT_SOCKET => self.socket_accept(a, b, c),
            abi::SEND_SOCKET => self.socket_write(a, b, c, d, e),
            abi::RECEIVE_SOCKET => self.socket_read(a, b, c, d, e),
            abi::SOCKET_ADDRESS => self.socket_name(c != 0, a, b),
            abi::SOCKET_OPTION => self.socket_option(a, b, c != 0, d),
            abi::CLOSE_SOCKET_WRITE => self
                .socket(a)
                .map_or_else(|error| error, |socket| socket.shutdown(1)),
            abi::DISPLAY_ACQUIRE if !self.identity.seat => EPERM,
            abi::DISPLAY_ACQUIRE
            | abi::DISPLAY_PRESENT
            | abi::DISPLAY_KEY
            | abi::DISPLAY_RELEASE
            | abi::DISPLAY_POINTER => super::graphics::call(tid, cr3, frame.rax, a),
            _ => ENOSYS,
        }
    }

    fn unlink(&self, dir: u64, address: u64) -> i64 {
        match self.path(dir, address) {
            Ok(path) if self.can_access(path.path(), true) => fs_result(ramfs::unlink(path.path())),
            Ok(_) => EACCES,
            Err(error) => error,
        }
    }

    fn rename(&self, old_dir: u64, old: u64, new_dir: u64, new: u64) -> i64 {
        let old = match self.path(old_dir, old) {
            Ok(path) => path,
            Err(error) => return error,
        };
        match self.path(new_dir, new) {
            Ok(path) if self.can_access(old.path(), true) && self.can_access(path.path(), true) => {
                fs_result(ramfs::rename(old.path(), path.path()))
            }
            Ok(_) => EACCES,
            Err(error) => error,
        }
    }

    fn read_directory(&mut self, fd: u64, address: u64) -> i64 {
        let cr3 = self.space.0;
        let slot = match self.regular_fd(fd) {
            Ok(slot) => slot,
            Err(error) => return error,
        };
        let directory = slot.file.as_ref().unwrap();
        if !directory.stat().0 {
            return ENOTDIR;
        }
        let Some(entry) = directory.directory_entry(slot.position as usize) else {
            return 0;
        };
        let mut record = [0; size_of::<abi::DirectoryEntry>()];
        record[..8].copy_from_slice(&entry.inode.to_ne_bytes());
        record[8..16].copy_from_slice(&(slot.position + 1).to_ne_bytes());
        let kind = if entry.directory {
            abi::FILE_DIRECTORY
        } else {
            abi::FILE_REGULAR
        };
        record[16..20].copy_from_slice(&kind.to_ne_bytes());
        record[20..24].copy_from_slice(&(entry.length as u32).to_ne_bytes());
        record[24..].copy_from_slice(&entry.name);
        if !memory::copy_out(cr3, address, &record) {
            return EFAULT;
        }
        slot.position += 1;
        1
    }

    fn make_dir(&self, dir: u64, address: u64) -> i64 {
        match self.path(dir, address) {
            Ok(path) if self.can_access(path.path(), true) => {
                fs_result(ramfs::create(path.path(), true))
            }
            Ok(_) => EACCES,
            Err(error) => error,
        }
    }

    fn remove_dir(&self, dir: u64, address: u64) -> i64 {
        match self.path(dir, address) {
            Ok(path) if self.can_access(path.path(), true) => {
                fs_result(ramfs::remove(path.path(), true))
            }
            Ok(_) => EACCES,
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

    fn handle_flags(&mut self, fd: u64, set: u64, flags: u64) -> i64 {
        if self.standard_open(fd) {
            return if set == 0 {
                (if fd == 0 {
                    abi::OPEN_READ
                } else {
                    abi::OPEN_WRITE
                }) as i64
            } else {
                0
            };
        }
        match self.descriptor(fd) {
            Ok(file) if set == 0 => {
                if let Some(socket) = file.socket {
                    (abi::OPEN_READ
                        | abi::OPEN_WRITE
                        | if socket.nonblocking {
                            abi::OPEN_NONBLOCK
                        } else {
                            0
                        }) as i64
                } else {
                    (file.flags
                        & (abi::OPEN_READ
                            | abi::OPEN_WRITE
                            | abi::OPEN_APPEND
                            | abi::OPEN_NONBLOCK)) as i64
                }
            }
            Ok(file) => {
                if let Some(socket) = &mut file.socket {
                    socket.nonblocking = flags & abi::OPEN_NONBLOCK != 0;
                } else {
                    file.flags = (file.flags & !(abi::OPEN_APPEND | abi::OPEN_NONBLOCK))
                        | flags & (abi::OPEN_APPEND | abi::OPEN_NONBLOCK);
                }
                0
            }
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

pub(super) fn fs_error(error: i64) -> i64 {
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

fn file_info(cr3: u64, address: u64, inode: u64, size: usize, kind: u32, owner: u32) -> i64 {
    let mut bytes = [0; size_of::<abi::FileInfo>()];
    bytes[..8].copy_from_slice(&inode.to_ne_bytes());
    bytes[8..16].copy_from_slice(&(size as u64).to_ne_bytes());
    bytes[16..20].copy_from_slice(&owner.to_ne_bytes());
    bytes[20..24].copy_from_slice(&kind.to_ne_bytes());
    out(cr3, address, &bytes, 0)
}

fn set_tls(context: *mut Context, address: u64) -> i64 {
    if address >= 0x8000_0000_0000 {
        return EPERM;
    }
    unsafe { (*context).fs_base = address };
    arch::set_fs_base(address);
    0
}

pub(super) fn ticks() -> u64 {
    unsafe { ptr::addr_of!(lapic_ticks_ms).read_volatile() }
}

fn sleep(milliseconds: u64) -> i64 {
    if milliseconds == 0 {
        arch::wait_for_interrupt();
        return 0;
    }
    if milliseconds > 86_400_000 {
        return EINVAL;
    }
    let end = ticks().saturating_add(milliseconds);
    while ticks() < end {
        arch::wait_for_interrupt();
    }
    0
}

fn getrandom(cr3: u64, address: u64, length: u64) -> i64 {
    let length = (length as usize).min(IO_MAX);
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

#[unsafe(no_mangle)]
unsafe extern "C" fn lumen_process_syscall(frame: *mut Frame) {
    let frame = unsafe { &mut *frame };
    let thread = unsafe { current_thread };
    if thread.is_null() || unsafe { (*thread).process }.is_null() {
        frame.rax = ENOSYS as u64;
        return;
    }
    frame.rax = unsafe { &mut *(*thread).process.cast::<Process>() }.call(frame, thread) as u64;
}
