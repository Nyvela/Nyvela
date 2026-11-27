use core::{ptr, slice};

const SDT_HEADER: usize = 36;
// Nyvela's boot page tables identity-map the first GiB.
const IDENTITY_END: usize = 1 << 30;
const MAX_TABLE_BYTES: usize = 1 << 20;

fn physical_bytes(address: usize, length: usize) -> Option<&'static [u8]> {
    if address == 0 || address.checked_add(length)? > IDENTITY_END {
        return None;
    }
    // Firmware tables remain reserved for the lifetime of the kernel.
    Some(unsafe { slice::from_raw_parts(ptr::with_exposed_provenance(address), length) })
}

fn le32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn le64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn checksum(bytes: &[u8]) -> bool {
    bytes.iter().copied().fold(0u8, u8::wrapping_add) == 0
}

fn find_rsdp(start: usize, end: usize) -> Option<&'static [u8]> {
    (start..end).step_by(16).find_map(|address| {
        let rsdp = physical_bytes(address, 20)?;
        (&rsdp[..8] == b"RSD PTR " && checksum(rsdp)).then_some(rsdp)
    })
}

fn table(address: usize, signature: &[u8; 4]) -> Option<&'static [u8]> {
    let header = physical_bytes(address, SDT_HEADER)?;
    let length = le32(header, 4) as usize;
    if &header[..4] != signature || !(SDT_HEADER..=MAX_TABLE_BYTES).contains(&length) {
        return None;
    }
    let full = physical_bytes(address, length)?;
    checksum(full).then_some(full)
}

fn find_madt(root: &[u8], width: usize) -> Option<&'static [u8]> {
    root[SDT_HEADER..].chunks_exact(width).find_map(|entry| {
        let address = if width == 8 {
            le64(entry, 0) as usize
        } else {
            le32(entry, 0) as usize
        };
        table(address, b"APIC")
    })
}

pub(crate) fn enabled_apic_ids() -> Option<[bool; 256]> {
    // BIOS data area word containing the EBDA segment.
    let ebda =
        usize::from(unsafe { ptr::read_volatile(ptr::without_provenance::<u16>(0x40e)) }) << 4;
    let rsdp = (0x80000..=0x9fc00)
        .contains(&ebda)
        .then(|| find_rsdp(ebda, ebda + 1024))
        .flatten()
        .or_else(|| find_rsdp(0xe0000, 0x100000))?;

    let xsdt = (rsdp[15] >= 2)
        .then(|| {
            let address = rsdp.as_ptr().addr();
            let extended = physical_bytes(address, 36)?;
            let length = le32(extended, 20) as usize;
            if !(36..=MAX_TABLE_BYTES).contains(&length)
                || !checksum(physical_bytes(address, length)?)
            {
                return None;
            }
            table(le64(extended, 24) as usize, b"XSDT")
        })
        .flatten();
    let madt = xsdt
        .and_then(|root| find_madt(root, 8))
        .or_else(|| find_madt(table(le32(rsdp, 16) as usize, b"RSDT")?, 4))?;
    parse_processors(madt.get(44..)?)
}

fn parse_processors(mut entries: &[u8]) -> Option<[bool; 256]> {
    let mut enabled = [false; 256];
    while !entries.is_empty() {
        let entry_type = entries[0];
        let length = usize::from(*entries.get(1)?);
        if length < 2 {
            return None;
        }
        let entry = entries.get(..length)?;
        match entry_type {
            0 if length >= 8 && le32(entry, 4) & 1 != 0 => {
                enabled[usize::from(entry[3])] = true;
            }
            9 if length >= 16 && le32(entry, 8) & 1 != 0 => {
                let id = le32(entry, 4) as usize;
                if id < 255 {
                    enabled[id] = true;
                }
            }
            _ => {}
        }
        entries = &entries[length..];
    }
    // The xAPIC destination value 0xff is reserved for broadcast.
    enabled[255] = false;
    Some(enabled)
}
