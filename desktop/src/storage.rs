//! Account and home snapshots over the existing Lumfat/NVMe transport.
use lamfat::{FileSystem, FsOptions, Read, Write};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{disk::Disk, sys};

const LIMIT: usize = 4 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"LUMDSK01";

fn disk_error() -> io::Error {
    io::Error::other("NVMe FAT32 volume unavailable")
}

fn access<T>(operation: impl FnOnce(&FileSystem<Disk<'_>>) -> io::Result<T>) -> io::Result<T> {
    let mut message = [0; 4112];
    let disk = Disk::open(&mut message).map_err(|_| disk_error())?;
    let volume = FileSystem::new(disk, FsOptions::new()).map_err(|_| disk_error())?;
    let value = operation(&volume)?;
    volume.unmount().map_err(|_| disk_error())?;
    Ok(value)
}

struct Snapshot {
    generation: u64,
    data: Vec<u8>,
}

enum Slot {
    Empty,
    Damaged,
    Valid(Snapshot),
}

fn read_slot(volume: &FileSystem<Disk<'_>>, name: &str) -> io::Result<Slot> {
    let root = volume.root_dir();
    let mut file = match root.open_file(name) {
        Ok(file) => file,
        Err(lamfat::Error::NotFound) => return Ok(Slot::Empty),
        Err(_) => return Err(disk_error()),
    };
    let mut data = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        let count = file.read(&mut chunk).map_err(|_| disk_error())?;
        if count == 0 {
            break;
        }
        if data.len() + count > LIMIT + 48 {
            return Ok(Slot::Damaged);
        }
        data.extend_from_slice(&chunk[..count]);
    }
    if data.len() < 48 || data.get(..8) != Some(MAGIC) {
        return Ok(Slot::Damaged);
    }
    let end = data.len() - 32;
    if Sha256::digest(&data[..end]).as_slice() != &data[end..] {
        return Ok(Slot::Damaged);
    }
    let generation = u64::from_le_bytes(data[8..16].try_into().unwrap());
    Ok(Slot::Valid(Snapshot {
        generation,
        data: data[16..end].to_vec(),
    }))
}

fn latest(
    volume: &FileSystem<Disk<'_>>,
    names: [&str; 2],
) -> io::Result<Option<(usize, Snapshot)>> {
    let first = read_slot(volume, names[0])?;
    let second = read_slot(volume, names[1])?;
    match (first, second) {
        (Slot::Valid(a), Slot::Valid(b)) => Ok(Some(if a.generation >= b.generation {
            (0, a)
        } else {
            (1, b)
        })),
        (Slot::Valid(a), _) => Ok(Some((0, a))),
        (_, Slot::Valid(b)) => Ok(Some((1, b))),
        (Slot::Empty, Slot::Empty) => Ok(None),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "both saved copies are unreadable",
        )),
    }
}

pub fn load(names: [&str; 2]) -> io::Result<Option<Vec<u8>>> {
    access(|volume| Ok(latest(volume, names)?.map(|(_, snapshot)| snapshot.data)))
}

pub fn save(names: [&str; 2], data: &[u8]) -> io::Result<()> {
    if data.len() > LIMIT {
        return Err(io::Error::other("snapshot exceeds 4 MiB"));
    }
    access(|volume| {
        let (slot, generation) = match latest(volume, names)? {
            Some((slot, snapshot)) => (
                1 - slot,
                snapshot.generation.checked_add(1).ok_or_else(disk_error)?,
            ),
            None => (0, 1),
        };
        let root = volume.root_dir();
        let mut file = root.create_file(names[slot]).map_err(|_| disk_error())?;
        let mut header = [0; 16];
        header[..8].copy_from_slice(MAGIC);
        header[8..].copy_from_slice(&generation.to_le_bytes());
        let mut hash = Sha256::new();
        hash.update(header);
        hash.update(data);
        file.write_all(&header).map_err(|_| disk_error())?;
        file.write_all(data).map_err(|_| disk_error())?;
        file.write_all(&hash.finalize()).map_err(|_| disk_error())?;
        file.truncate().map_err(|_| disk_error())?;
        file.flush().map_err(|_| disk_error())
    })
}

fn home_names(uid: u32) -> [String; 2] {
    [format!("u{uid:05}a.dat"), format!("u{uid:05}b.dat")]
}

pub fn backup_home(uid: u32, home: &Path) -> io::Result<()> {
    let mut output = Vec::new();
    pack(home, home, &mut output, 0)?;
    let names = home_names(uid);
    save([names[0].as_str(), names[1].as_str()], &output)
}

fn pack(root: &Path, dir: &Path, output: &mut Vec<u8>, depth: usize) -> io::Result<()> {
    if depth >= 16 {
        return Err(io::Error::other("home directory nesting exceeds 16 levels"));
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|_| disk_error())?;
        let name = relative
            .to_str()
            .ok_or_else(|| io::Error::other("unsupported file name"))?
            .as_bytes();
        let directory = entry.file_type()?.is_dir();
        let length = if directory {
            0
        } else {
            entry.metadata()?.len() as usize
        };
        let size = 7_usize
            .checked_add(name.len())
            .and_then(|n| n.checked_add(length))
            .ok_or_else(disk_error)?;
        if name.len() > 200 || size > LIMIT.saturating_sub(output.len()) {
            return Err(io::Error::other("home snapshot exceeds its size limit"));
        }
        output.push(u8::from(directory));
        output.extend_from_slice(&(name.len() as u16).to_le_bytes());
        output.extend_from_slice(&(length as u32).to_le_bytes());
        output.extend_from_slice(name);
        if directory {
            pack(root, &path, output, depth + 1)?;
        } else {
            let data = fs::read(&path)?;
            if data.len() != length {
                return Err(io::Error::other("home changed during backup"));
            }
            output.extend_from_slice(&data);
        }
    }
    Ok(())
}

pub fn restore_home(uid: u32, home: &Path) -> io::Result<()> {
    let names = home_names(uid);
    let Some(data) = load([names[0].as_str(), names[1].as_str()])? else {
        return Ok(());
    };
    for (path, directory, contents) in records(&data)? {
        let path = home.join(path);
        if directory {
            fs::create_dir_all(path)?;
        } else {
            fs::write(path, contents)?;
        }
    }
    Ok(())
}

fn records(mut data: &[u8]) -> io::Result<Vec<(PathBuf, bool, &[u8])>> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid home snapshot");
    let mut records = Vec::new();
    while !data.is_empty() {
        let header = data.get(..7).ok_or_else(invalid)?;
        let directory = header[0] == 1;
        let name_len = u16::from_le_bytes([header[1], header[2]]) as usize;
        let size = u32::from_le_bytes(header[3..7].try_into().unwrap()) as usize;
        if header[0] > 1 || (directory && size != 0) || name_len == 0 || name_len > 200 {
            return Err(invalid());
        }
        let end = 7 + name_len + size;
        let record = data.get(7..end).ok_or_else(invalid)?;
        let name = std::str::from_utf8(&record[..name_len]).map_err(|_| invalid())?;
        if name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.contains('\0'))
        {
            return Err(invalid());
        }
        records.push((PathBuf::from(name), directory, &record[name_len..]));
        data = &data[end..];
    }
    Ok(records)
}

pub fn wait_for_volume() -> io::Result<()> {
    let deadline = sys::uptime().saturating_add(20_000);
    loop {
        match access(|_| Ok(())) {
            Ok(()) => return Ok(()),
            Err(error) if sys::uptime() >= deadline => return Err(error),
            Err(_) => sys::sleep(100),
        }
    }
}
