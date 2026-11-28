use std::io;

use crate::{abi, trap};

pub fn call(operation: u64, values: [u64; 6]) -> i64 {
    unsafe { trap::call(operation, values) }
}

pub fn checked(result: i64) -> io::Result<u64> {
    if result < 0 {
        Err(io::Error::from_raw_os_error(-result as i32))
    } else {
        Ok(result as u64)
    }
}

pub fn identity(operation: u64) -> u64 {
    checked(call(abi::IDENTITY, [operation, 0, 0, 0, 0, 0])).unwrap_or_default()
}

pub fn uptime() -> u64 {
    call(abi::UPTIME, [0; 6]).max(0) as u64
}

pub fn sleep(milliseconds: u64) {
    call(abi::SLEEP, [milliseconds, 0, 0, 0, 0, 0]);
}

pub fn send(tid: usize, bytes: &[u8]) -> Result<(), ()> {
    match call(
        abi::SEND,
        [
            tid as u64,
            bytes.as_ptr().addr() as u64,
            bytes.len() as u64,
            0,
            0,
            0,
        ],
    ) {
        1 => Ok(()),
        _ => Err(()),
    }
}

pub fn message(bytes: &mut [u8]) -> Option<(usize, usize)> {
    let mut sender = 0;
    let length = call(
        abi::RECEIVE,
        [
            bytes.as_mut_ptr().addr() as u64,
            bytes.len() as u64,
            (&raw mut sender).addr() as u64,
            0,
            0,
            0,
        ],
    );
    (length > 0).then_some((sender, length as usize))
}

pub fn receive(tid: usize, output: &mut [u8]) -> Result<usize, ()> {
    let deadline = uptime().saturating_add(5_000);
    loop {
        if let Some((sender, size)) = message(output)
            && sender == tid
        {
            return Ok(size);
        }
        if uptime() >= deadline {
            return Err(());
        }
        sleep(1);
    }
}

pub fn exchange(tid: usize, request: &[u8], response: &mut [u8]) -> Result<usize, ()> {
    send(tid, request)?;
    receive(tid, response)
}

pub const SEAT: u64 = 1;
pub const CAPTURE: u64 = 2;

pub fn spawn(path: &str, arguments: &[&str], uid: u64, flags: u64) -> io::Result<u64> {
    let program = std::ffi::CString::new(path)?;
    let mut packed = Vec::new();
    for &argument in arguments {
        if argument.as_bytes().contains(&0) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        packed.extend_from_slice(argument.as_bytes());
        packed.push(0);
    }
    checked(call(
        abi::SPAWN,
        [
            program.as_ptr().addr() as u64,
            packed.as_ptr().addr() as u64,
            packed.len() as u64,
            uid,
            flags,
            0,
        ],
    ))
}

pub fn wait(tid: u64, nonblocking: bool) -> io::Result<u64> {
    checked(call(abi::WAIT, [tid, u64::from(nonblocking), 0, 0, 0, 0]))
}

pub fn random(bytes: &mut [u8]) -> io::Result<()> {
    let count = checked(call(
        abi::RANDOM,
        [
            bytes.as_mut_ptr().addr() as u64,
            bytes.len() as u64,
            0,
            0,
            0,
            0,
        ],
    ))?;
    if count != bytes.len() as u64 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    Ok(())
}

pub fn register(account: &abi::Account) -> io::Result<()> {
    checked(call(
        abi::ACCOUNT_REGISTER,
        [
            (account as *const abi::Account).addr() as u64,
            0,
            0,
            0,
            0,
            0,
        ],
    ))?;
    Ok(())
}
