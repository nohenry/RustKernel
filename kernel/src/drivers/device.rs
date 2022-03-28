use alloc::{string::String, vec::Vec};
use aml::{resource::Resource, AmlValue};
use spin::Mutex;

pub enum DeviceType {
    PciDevice {
        segment: u16,
        bus: u8,
        device: u8,
        function: u8,
    },
    Generic {
        hid: AmlValue,
    },
}

pub struct Device {
    crate device_type: DeviceType,
    crate resource: Vec<Resource>,
}

// pub static GLOBAL_DEVICES: Mutex<Vec<Device>> = Mutex::new(Vec::new());
static mut DEVICES: Vec<Device> = Vec::new();

pub fn add_device(device: Device) {
    unsafe {
        DEVICES.push(device);
    }
}

pub fn search_pci_device(
    _segment: u16,
    _bus: u8,
    _device: u8,
    _function: u8,
) -> Option<&'static Device> {
    let devices = unsafe { &DEVICES };
    // let devices = GLOBAL_DEVICES.lock();
    let device = devices.iter().find(|d| match d.device_type {
        DeviceType::PciDevice {
            segment,
            bus,
            device,
            function,
        } => {
            if (segment, bus, device, function) == (_segment, _bus, _device, _function) {
                return true;
            }
            false
        }
        _ => false,
    });
    device
}
