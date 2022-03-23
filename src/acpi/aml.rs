use alloc::boxed::Box;
use aml::{
    name_object::NameSeg, resource::resource_descriptor_list, AmlContext, DebugVerbosity, Handler,
};

use crate::{
    drivers::pci::{get_pci, get_pci_mut},
    util::{in16, in32, in8, out16, out32, out8},
};

use super::{fadt::FADT, get_xsdt, xsdt::Header, Signature};

struct AmlHandler;

impl Handler for AmlHandler {
    fn read_u8(&self, address: usize) -> u8 {
        unsafe { core::ptr::read_volatile(address as *const _) }
    }

    fn read_u16(&self, address: usize) -> u16 {
        unsafe { core::ptr::read_volatile(address as *const _) }
    }

    fn read_u32(&self, address: usize) -> u32 {
        unsafe { core::ptr::read_volatile(address as *const _) }
    }

    fn read_u64(&self, address: usize) -> u64 {
        unsafe { core::ptr::read_volatile(address as *const _) }
    }

    fn write_u8(&mut self, address: usize, value: u8) {
        unsafe { core::ptr::write_volatile(address as *mut _, value) }
    }

    fn write_u16(&mut self, address: usize, value: u16) {
        unsafe { core::ptr::write_volatile(address as *mut _, value) }
    }

    fn write_u32(&mut self, address: usize, value: u32) {
        unsafe { core::ptr::write_volatile(address as *mut _, value) }
    }

    fn write_u64(&mut self, address: usize, value: u64) {
        unsafe { core::ptr::write_volatile(address as *mut _, value) }
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { in8(port) }
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { in16(port) }
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { in32(port) }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { out8(port, value) }
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { out16(port, value) }
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { out32(port, value) }
    }

    fn read_pci_u8(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u8 {
        get_pci().read_u8(segment, bus, device, function, offset)
    }

    fn read_pci_u16(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u16 {
        get_pci().read_u16(segment, bus, device, function, offset)
    }

    fn read_pci_u32(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u32 {
        get_pci().read_u32(segment, bus, device, function, offset)
    }

    fn write_pci_u8(
        &self,
        segment: u16,
        bus: u8,
        device: u8,
        function: u8,
        offset: u16,
        value: u8,
    ) {
        get_pci_mut().write_u8(segment, bus, device, function, offset, value);
    }

    fn write_pci_u16(
        &self,
        segment: u16,
        bus: u8,
        device: u8,
        function: u8,
        offset: u16,
        value: u16,
    ) {
        get_pci_mut().write_u16(segment, bus, device, function, offset, value);
    }

    fn write_pci_u32(
        &self,
        segment: u16,
        bus: u8,
        device: u8,
        function: u8,
        offset: u16,
        value: u32,
    ) {
        get_pci_mut().write_u32(segment, bus, device, function, offset, value);
    }
}

pub fn init() {
    let xsdt = get_xsdt();
    let fadt = xsdt
        .iter()
        .find(|table| table.signature == Signature::FADT.as_bytes())
        .expect("Unable to get FADT!");

    let dsdt = unsafe { &*fadt.get_entry::<FADT>().dsdt };
    let b = unsafe {
        core::slice::from_raw_parts(
            &dsdt.data,
            dsdt.header.length as usize - core::mem::size_of::<Header>(),
        )
    };
    kprintln!("{}", b.len());

    let dump = unsafe {
        core::slice::from_raw_parts(
            &dsdt.header as *const _ as *const u8,
            dsdt.header.length as _,
        )
    };
    for b in dump {
        kprint!("{:02x}", b);
    }
loop {}

    let mut aml_context = AmlContext::new(Box::new(AmlHandler), DebugVerbosity::All);
    aml_context.parse_table(b).unwrap();
    aml_context.initialize_objects().unwrap();
    // let mut level = None;
    // aml_context
    //     .namespace
    //     .traverse(|name, nlevel| {
    //         if name.as_string() == "\\_SB_.PCI0" {
    //             level = Some(nlevel.clone());
    //         }
    //         Ok(true)
    //     })
    //     .unwrap();

    kprintln!("{:#x?}", aml_context.namespace);

    // kprintln!("\n\n\nLevel{:#?}", level);
    let rt = &aml_context.namespace;
    let mut rt = rt.clone();
    rt.traverse(|n, l| {
        kprintln!("name: {}", n);

        if let Some(handle) = l.values.get(&NameSeg::from_str("_HID").unwrap()) {
            let res = aml_context.namespace.get(*handle).unwrap();
            kprintln!("  HID {:x?}", res);
        }
        if let Some(handle) = l.values.get(&NameSeg::from_str("_UID").unwrap()) {
            let res = aml_context.namespace.get(*handle).unwrap();
            kprintln!("  UID {:x?}", res);
        }
        if let Some(handle) = l.values.get(&NameSeg::from_str("_CRS").unwrap()) {
            let res = aml_context.namespace.get(*handle);
            match res {
                Ok(val) => {
                    let resources = resource_descriptor_list(val);
                    match resources {
                        Ok(resources) => {
                            for res in resources {
                                kprintln!("  Handle: {:x?}", res);
                            }
                        }
                        _ => (),
                    }
                }
                Err(_e) => {
                    kprintln!("Unable to find handle!");
                }
            }
        }
        Ok(true)
    })
    .unwrap();
    // for (cname, clevel) in &level.unwrap().children {
    // }
    // aml_context.namespace.get_handle()
}
