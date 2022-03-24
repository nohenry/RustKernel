use alloc::boxed::Box;
use aml::{
    name_object::NameSeg, pci_routing::PciRoutingTable, resource::resource_descriptor_list,
    value::Args, AmlContext, AmlName, AmlValue, DebugVerbosity, Handler, LevelType,
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

pub static mut GLOBAL_AML: Option<AmlContext> = None;

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

    let mut aml_context = AmlContext::new(Box::new(AmlHandler), DebugVerbosity::All);
    aml_context.parse_table(b).unwrap();
    aml_context.initialize_objects().unwrap();
    // let mut level = None;
    aml_context
        .invoke_method(
            &AmlName::from_str("\\_PIC").unwrap(),
            Args([
                Some(AmlValue::Integer(1)),
                None,
                None,
                None,
                None,
                None,
                None,
            ]),
        )
        .unwrap();

    let namespace = &aml_context.namespace;
    let namespace = namespace.clone();
    aml_context
        .namespace
        .traverse(|name, nlevel| {
            if let LevelType::Device = nlevel.typ {
                kprintln!("Device: {}", name);
                // kprintln!("  {:?}", nlevel);

                let hid = nlevel.values.get(&NameSeg::from_str("_HID").unwrap());
                let crs = nlevel.values.get(&NameSeg::from_str("_CRS").unwrap());
                if let Some(hid) = hid {
                    let hid = namespace.get(*hid).unwrap();
                    kprintln!("  HID: {:x?}", hid);
                }
                if let Some(crs) = crs {
                    let crs = namespace.get(*crs).unwrap();
                    let crs_resources = resource_descriptor_list(crs);
                    if let Ok(ref crs_resources) = crs_resources {
                        for res in crs_resources.iter() {
                            kprintln!(" CRS: {:x?}", res)
                        }
                    }
                }
            }
            Ok(true)
        })
        .unwrap();

    kprintln!("{:#x?}", aml_context.namespace);


    unsafe {
        GLOBAL_AML = Some(aml_context);
    }

}
