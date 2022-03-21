use core::marker::PhantomData;

use super::xsdt::{Header, XSDTEntry};

#[repr(C, packed)]
pub struct MADT {
    header: Header,
    apic_address: u32,
    flags: u32,
    entries: Entry,
}

impl MADT {
    pub fn iter(&self) -> EntryIterator {
        EntryIterator {
            entry_base: unsafe { &self.entries as *const _ as *const u8 },
            index: 0,
            size: self.header.length - 44,
            pd: PhantomData,
        }
    }
}

impl XSDTEntry for MADT {}

#[repr(u8)]
#[derive(Debug)]
pub enum Entry {
    LocalApic {
        length: u8,
        processor_id: u8,
        apic_id: u8,
        flags: u32,
    } = 0,
    IoApic {
        length: u8,
        io_apic_id: u8,
        _reserved: u8,
        io_apic_address: u32,
        gsi_base: u32,
    },
    IoApicOverride {
        length: u8,
        bus: u8,
        irq: u8,
        gsi: u32,
        flags: u16,
    },
    IoApicNMI {
        length: u8,
        source: u8,
        _reserved: u8,
        flags: u16,
        gsi: u32,
    },
    LocalApicOverride {
        length: u8,
        _reserved: u16,
        address: u64,
    },
    LocalX2Apic {
        length: u8,
        _reserved: u16,
        x2_id: u32,
        flags: u32,
        acpi_id: u32,
    } = 9,
}

pub struct EntryIterator<'a> {
    entry_base: *const u8,
    index: u32,
    size: u32,
    pd: PhantomData<&'a ()>,
}

impl<'a> Iterator for EntryIterator<'a> {
    type Item = &'a Entry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            let entry = unsafe { &*(self.entry_base.offset(self.index as _) as *const Entry) };
            match entry {
                Entry::LocalApic { length, .. } => {
                    self.index += *length as u32;
                }
                Entry::IoApic { length, .. } => {
                    self.index += *length as u32;
                }
                Entry::IoApicOverride { length, .. } => {
                    self.index += *length as u32;
                }
                Entry::IoApicNMI { length, .. } => {
                    self.index += *length as u32;
                }
                Entry::LocalApicOverride { length, .. } => {
                    self.index += *length as u32;
                }
                Entry::LocalX2Apic { length, .. } => {
                    self.index += *length as u32;
                }
            }
            Some(entry)
        } else {
            None
        }
    }
}
