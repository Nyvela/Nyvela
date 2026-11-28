//! PCI configuration mechanism #1 on the boot CPU.
use core::arch::asm;

pub struct Device(u32);

pub fn locate(vendor: u16, device: u16) -> Option<Device> {
    let identity = u32::from(vendor) | (u32::from(device) << 16);
    // The supported QEMU devices are on the primary PCI bus.
    (0..256)
        .map(|function| Device(0x8000_0000 | function << 8))
        .find(|candidate| candidate.read(0) == identity)
}

impl Device {
    fn select(&self, register: u8) {
        let selector = self.0 | u32::from(register & !3);
        unsafe {
            asm!("out dx, eax", in("dx") 0xcf8_u16, in("eax") selector, options(nomem, nostack, preserves_flags));
        }
    }

    pub fn read(&self, register: u8) -> u32 {
        self.select(register);
        let value;
        unsafe {
            asm!("in eax, dx", in("dx") 0xcfc_u16, out("eax") value, options(nomem, nostack, preserves_flags));
        }
        value
    }

    pub fn enable(&self, flags: u16) {
        let command = self.read(4) as u16 | flags;
        // Write only the command register. The upper half is PCI status.
        unsafe {
            asm!("out dx, ax", in("dx") 0xcfc_u16, in("ax") command, options(nomem, nostack, preserves_flags));
        }
    }
}
