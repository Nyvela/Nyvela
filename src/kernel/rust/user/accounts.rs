//! Session identities are installed by the privileged login manager. Password
//! verification and the on-disk account database belong to that manager.
use crate::{abi, ramfs};

use super::memory::{self, WRITE};

const MAX_USERS: usize = 16;
pub const ROOT: abi::Account = abi::Account {
    uid: 0,
    admin: 1,
    name: *b"root\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    reserved: [0; 3],
};

static mut ACCOUNTS: [abi::Account; MAX_USERS] = [abi::Account {
    uid: 0,
    admin: 0,
    name: [0; 24],
    reserved: [0; 3],
}; MAX_USERS];

pub fn lookup(uid: u32) -> Option<abi::Account> {
    if uid == 0 {
        return Some(ROOT);
    }
    let entries = unsafe { &*(&raw const ACCOUNTS) };
    entries.iter().copied().find(|entry| entry.uid == uid)
}

pub fn home(account: abi::Account, output: &mut [u8; 32]) -> usize {
    if account.uid == 0 {
        output[0] = b'/';
        return 1;
    }
    let length = account
        .name
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(account.name.len());
    output[..6].copy_from_slice(b"/home/");
    output[6..6 + length].copy_from_slice(&account.name[..length]);
    6 + length
}

pub fn owner(path: &[u8]) -> u32 {
    let Some(home) = path.strip_prefix(b"/home/") else {
        return 0;
    };
    let name = home.split(|&byte| byte == b'/').next().unwrap_or_default();
    if name.is_empty() || name.len() >= 24 {
        return 0;
    }
    let entries = unsafe { &*(&raw const ACCOUNTS) };
    entries
        .iter()
        .find(|entry| {
            entry.uid != 0 && entry.name[..name.len()] == *name && entry.name[name.len()] == 0
        })
        .map_or(0, |entry| entry.uid)
}

pub fn register(cr3: u64, input: u64) -> i64 {
    let mut bytes = [0; 32];
    if !memory::copy_in(cr3, input, &mut bytes) {
        return -14;
    }
    let uid = u32::from_ne_bytes(bytes[..4].try_into().unwrap());
    let mut entry = abi::Account {
        uid,
        admin: bytes[4],
        name: [0; 24],
        reserved: [0; 3],
    };
    entry.name.copy_from_slice(&bytes[5..29]);
    let Some(length) = entry.name.iter().position(|&byte| byte == 0) else {
        return -22;
    };
    let username = &entry.name[..length];
    if !(1000..=65535).contains(&uid)
        || entry.admin > 1
        || !username.first().is_some_and(|c| c.is_ascii_lowercase())
        || username == b"root"
        || username
            .iter()
            .any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && *c != b'-')
        || entry.name[length..].iter().any(|&byte| byte != 0)
    {
        return -22;
    }

    let entries = unsafe { &mut *(&raw mut ACCOUNTS) };
    if let Some(previous) = entries
        .iter()
        .find(|previous| previous.uid == uid || previous.name == entry.name)
    {
        return if previous.uid == uid
            && previous.name == entry.name
            && previous.admin == entry.admin
        {
            0
        } else {
            -17
        };
    }
    let Some(slot) = entries.iter_mut().find(|entry| entry.uid == 0) else {
        return -28;
    };
    let mut path = [0; 32];
    let length = home(entry, &mut path);
    for directory in [b"/home".as_slice(), &path[..length]] {
        match ramfs::create(directory, true) {
            0 => {}
            -3 if matches!(ramfs::stat(directory), Ok((true, _))) => {}
            _ => return -28,
        }
    }
    *slot = entry;
    0
}

pub fn describe(account: abi::Account, cr3: u64, address: u64) -> i64 {
    let mut bytes = [0; 32];
    if !memory::valid(cr3, address, bytes.len(), WRITE) {
        return -14;
    }
    bytes[..4].copy_from_slice(&account.uid.to_ne_bytes());
    bytes[4] = account.admin;
    bytes[5..29].copy_from_slice(&account.name);
    memory::copy_out(cr3, address, &bytes);
    0
}
