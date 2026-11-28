//! Lumen x86-64 application ABI. EAX selects the operation, the arguments
//! use RDI, RSI, RDX, R10, R8 and R9, and RAX holds the signed result.
#![allow(dead_code)]

pub const EXIT: u64 = 0;
pub const WRITE: u64 = 1;
pub const YIELD: u64 = 2;
pub const READ: u64 = 3;
pub const SEND: u64 = 5;
pub const RECEIVE: u64 = 6;
pub const SELF: u64 = 47;
pub const UPTIME: u64 = 48;

pub const OPEN: u64 = 0x100;
pub const CLOSE: u64 = 0x101;
pub const READ_AT: u64 = 0x102;
pub const WRITE_AT: u64 = 0x103;
pub const INFO_PATH: u64 = 0x104;
pub const INFO_HANDLE: u64 = 0x105;
pub const SEEK: u64 = 0x106;
pub const RESIZE_FILE: u64 = 0x107;
pub const SYNC_FILE: u64 = 0x108;
pub const CREATE_DIRECTORY: u64 = 0x109;
pub const REMOVE_PATH: u64 = 0x10a;
pub const MOVE_PATH: u64 = 0x10b;
pub const READ_DIRECTORY: u64 = 0x10c;
pub const WORKING_DIRECTORY: u64 = 0x10d;
pub const HANDLE_FLAGS: u64 = 0x10e;
pub const POLL: u64 = 0x10f;

pub const MAP_MEMORY: u64 = 0x120;
pub const UNMAP_MEMORY: u64 = 0x121;
pub const PROTECT_MEMORY: u64 = 0x122;
pub const BREAK: u64 = 0x123;
pub const SET_TLS: u64 = 0x124;
pub const GET_TLS: u64 = 0x125;
pub const RANDOM: u64 = 0x126;
pub const SLEEP: u64 = 0x127;

pub const OPEN_SOCKET: u64 = 0x140;
pub const BIND_SOCKET: u64 = 0x141;
pub const CONNECT_SOCKET: u64 = 0x142;
pub const LISTEN_SOCKET: u64 = 0x143;
pub const ACCEPT_SOCKET: u64 = 0x144;
pub const SEND_SOCKET: u64 = 0x145;
pub const RECEIVE_SOCKET: u64 = 0x146;
pub const SOCKET_ADDRESS: u64 = 0x147;
pub const SOCKET_OPTION: u64 = 0x148;
pub const CLOSE_SOCKET_WRITE: u64 = 0x149;

pub const DISPLAY_ACQUIRE: u64 = 0x160;
pub const DISPLAY_PRESENT: u64 = 0x161;
pub const DISPLAY_KEY: u64 = 0x162;
pub const DISPLAY_RELEASE: u64 = 0x163;
pub const DISPLAY_POINTER: u64 = 0x164;

pub const SPAWN: u64 = 0x170;
pub const WAIT: u64 = 0x171;
pub const IDENTITY: u64 = 0x172;
pub const ACCOUNT_REGISTER: u64 = 0x174;
pub const ACCOUNT_DESCRIBE: u64 = 0x175;

pub const OPEN_READ: u64 = 1;
pub const OPEN_WRITE: u64 = 2;
pub const OPEN_CREATE: u64 = 4;
pub const OPEN_EXCLUSIVE: u64 = 8;
pub const OPEN_TRUNCATE: u64 = 16;
pub const OPEN_APPEND: u64 = 32;
pub const OPEN_DIRECTORY: u64 = 64;
pub const OPEN_NONBLOCK: u64 = 128;
pub const CURRENT_DIRECTORY: u64 = u64::MAX;
pub const MAP_FIXED: u64 = 1;
pub const SOCKET_STREAM: u64 = 1;
pub const SOCKET_DATAGRAM: u64 = 2;
pub const NONBLOCK: u64 = 1;
pub const SOCKET_KIND: u64 = 1;
pub const SOCKET_ERROR: u64 = 2;
pub const SOCKET_REUSE: u64 = 3;
pub const SOCKET_READ_TIMEOUT: u64 = 4;
pub const SOCKET_WRITE_TIMEOUT: u64 = 5;

pub const FILE_REGULAR: u32 = 0;
pub const FILE_DIRECTORY: u32 = 1;
pub const FILE_CONSOLE: u32 = 2;
pub const FILE_SOCKET: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FileInfo {
    pub inode: u64,
    pub size: u64,
    pub owner: u32,
    pub kind: u32,
}

#[repr(C)]
pub struct DirectoryEntry {
    pub inode: u64,
    pub next: u64,
    pub kind: u32,
    pub length: u32,
    pub name: [u8; 64],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Endpoint {
    pub ip: [u8; 4],
    pub port: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Pointer {
    pub dx: i16,
    pub dy: i16,
    pub buttons: u8,
    pub reserved: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Account {
    pub uid: u32,
    pub admin: u8,
    pub name: [u8; 24],
    pub reserved: [u8; 3],
}

pub const DISPLAY_WIDTH: usize = 640;
pub const DISPLAY_HEIGHT: usize = 400;
pub const IPC_MAX: usize = 4112;
