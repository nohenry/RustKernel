use alloc::vec::{self, Vec};
use aml::{pci_routing::{PciRoutingTable, Pin}, AmlName, resource::{Resource, IOPortDescriptor, MemoryRangeDescriptor}};
use bit_field::BitField;

use crate::{
    acpi::{get_xsdt, mcfg::MCFG, Signature, aml::GLOBAL_AML},
    drivers::device::{Device, DeviceType, self},
    kprintln,
};

// use super::device::GLOBAL_DEVICES;

// pub static GLOBAL_PCI: AtomicPtr<PCI> = AtomicPtr::new(core::ptr::null_mut());
pub static mut GLOBAL_PCI: PCI = PCI::new();

pub fn init() {
    let xsdt = get_xsdt();
    let mcfg = xsdt
        .iter()
        .find(|table| table.signature == Signature::MCFG.as_bytes())
        .expect("Unable to get MCFG!");
    let mcfg = mcfg.get_entry::<MCFG>();

    unsafe {
        GLOBAL_PCI.mcfg = Some(mcfg);
    }
}

pub fn gather_devices() {
    unsafe {

        GLOBAL_PCI.traverse_devices(|segment, bus, device, function| {
            let class = GLOBAL_PCI.read_u8(segment, bus, device, function, PCI::CLASS);
            let subclass = GLOBAL_PCI.read_u8(segment, bus, device, function, PCI::SUBCLASS);
            let deviceid = GLOBAL_PCI.read_u16(segment, bus, device, function, PCI::DEVICE_ID);
            let vendor = GLOBAL_PCI.read_u16(segment, bus, device, function, PCI::VENDOR_ID);
            let intpin = GLOBAL_PCI.read_u8(segment, bus, device, function, PCI::INT_LINE);
            let intline = GLOBAL_PCI.read_u8(segment, bus, device, function, PCI::INT_PIN);
            kprintln!(
                "Device [Bus: {}, Device: {}, Function: {}, Type: {}, Id: {:x}, Vendor: {:x}, Int Pin: {}, Int Line: {}]",
                bus,
                device,
                function,
                class_str(class, subclass, 0),
                deviceid,
                vendor,
                intpin,
                intline
            );

            let table = PciRoutingTable::from_prt_path(
               &AmlName::from_str("\\_SB_.PCI0._PRT").unwrap(),
                &mut GLOBAL_AML.as_mut().unwrap(),
                    )
                    .unwrap();

            let mut resources = Vec::new();
            if intline > 0 {
                let pin = match intline {
                    1 => Pin::IntA,
                    2 => Pin::IntB,
                    3 => Pin::IntC,
                    4 => Pin::IntD,
                    _ => panic!("Unkown pin!")
                };
                let route = table.route(device as _, function as _, pin, &mut GLOBAL_AML.as_mut().unwrap()).expect("Unable to map pci to interrupt");
                kprintln!("  Interrupt: {:?}", route);
                resources.push(Resource::Irq(route));
            }

            let mut bars = GLOBAL_PCI.get_bars(segment, bus, device, function);
            for bar in bars.iter() {
                kprintln!("  Bar: {:x?}", bar);
            }
            resources.append(&mut bars);

            
            device::add_device(
                Device {
                    device_type: DeviceType::PciDevice {segment, bus, device, function},
                    resource: resources,
                }
            );
        });
    }
}

pub fn get_pci() -> &'static PCI<'static> {
    unsafe { &GLOBAL_PCI }
}

pub fn get_pci_mut() -> &'static mut PCI<'static> {
    unsafe { &mut GLOBAL_PCI }
}

#[repr(C, packed)]
pub struct BAR {
    pub data: u32,
}

impl BAR {
    fn used(&self) -> bool {
        self.data != 0
    }

    fn is_io(&self) -> bool {
        self.data.get_bit(0)
    }

    fn is_mem(&self) -> bool {
        !self.data.get_bit(0)
    }

    fn is_prefetch(&self) -> bool {
        self.data.get_bit(3)
    }

    fn get_address(&self) -> u32 {
        self.data & 0xFFFFFFF0
        // self.data.get_bits(4..=31)
    }

    fn get_io(&self) -> u32 {
        self.data & 0xFFFFFFFC
    }

    fn is_64bit(&self) -> bool {
        self.data.get_bits(1..=2) == 0x02
    }
}

impl From<u32> for BAR {
    fn from(data: u32) -> Self {
        BAR {
            data
        }
    }
}

pub struct PCI<'a> {
    pub(self) mcfg: Option<&'a MCFG<'a>>,
}

impl<'a> PCI<'a> {
    const VENDOR_ID: u16 = 0x00;
    const DEVICE_ID: u16 = 0x02;
    const COMMAND: u16 = 0x04;
    const STATUS: u16 = 0x06;
    const REVISION: u16 = 0x08;
    const PROG_IF: u16 = 0x09;
    const SUBCLASS: u16 = 0x0A;
    const CLASS: u16 = 0x0B;
    const CACHE_SIZE: u16 = 0x0C;
    const LATENCY_TIMER: u16 = 0x0D;

    const TYPE: u16 = 0x0E;

    const BIST: u16 = 0x0F;
    const BAR0: u16 = 0x10;
    const BAR1: u16 = 0x14;
    const BAR2: u16 = 0x18;
    const BAR3: u16 = 0x1C;
    const BAR4: u16 = 0x20;
    const BAR5: u16 = 0x24;

    const PRIMARY_BUS: u16 = 0x18;
    const SECONDARY_BUS: u16 = 0x19;
    const SUBORDINATE_BUS: u16 = 0x1A;
    const SECONDARY_LATENCY_TIMER: u16 = 0x1B;
    const IOBASE: u16 = 0x1C;
    const IOLIMIT: u16 = 0x1D;
    const SECONDARY_STATUS: u16 = 0x1E;
    const MEMORY_BASE: u16 = 0x20;
    const MEMORY_LIMIT: u16 = 0x22;
    const PREFETCH_MEMORY_BASE: u16 = 0x24;
    const PREFETCH_MEMORY_LIMIT: u16 = 0x26;
    const PREFETCH_BASE_UPPER: u16 = 0x28;
    const PREFETCH_LIMIT_UPPER: u16 = 0x2C;
    const IOBASE_UPPER: u16 = 0x30;
    const IOLIMIT_UPPER: u16 = 0x30;
    const CAPABILITY: u16 = 0x38;
    const INT_LINE: u16 = 0x3C;
    const INT_PIN: u16 = 0x3D;
    const BRIDGE_CTL: u16 = 0x3E;

    const fn new() -> PCI<'a> {
        PCI { mcfg: None }
    }

    fn get_bars(&mut self, segment: u16, bus: u8, device: u8, function: u8) -> Vec<Resource> {
        let ptype = self.read_u8(segment, bus, device, function, PCI::TYPE) & 0x7F;
        // let mut ress: [Option<Resource>; 6] = Default::default();
        // let mut res_index = 0;
        let mut resources = Vec::new();

        let mut devs = |to_val: u16| {
            let mut i = 0; 
            while i < to_val {
                    let s = PCI::BAR0 + 4 * i;
                    let o = PCI::BAR0 + 4 * (i + 1); // Store the next bar for 64 bit addresses
    
                    // Read first bar and it's length
                    let bar = BAR::from(self.read_u32(segment, bus, device, function, s));
                    self.write_u32(segment, bus, device, function, s, u32::MAX);
                    let barl = self.read_u32(segment, bus, device, function, s);
                    
                    let res = if bar.used() {
                        if bar.is_io() {
                            let length = (!(barl & 0xFFFFFFFC) + 1) as u16;
                            Some(Resource::IOPort(IOPortDescriptor {
                                memory_range: (bar.get_io() as _, bar.get_io() as u16 + length - 1),
                                base_alignment: 1,
                                decodes_full_address: false,
                                range_length: length as _
                            }))
                        } else {
                            if bar.is_64bit() {
                                let nar = BAR::from(self.read_u32(segment, bus, device, function, o));
                                self.write_u32(segment, bus, device, function, o, u32::MAX);
                                let narl = self.read_u32(segment, bus, device, function, o) as u64;
                                i += 1;

                                let length = (!(barl & 0xFFFFFFF0) + 1) as u64;
                                let nlength = (!(narl & 0xFFFFFFF0) + 1) as u64;

                                Some(Resource::MemoryRange(MemoryRangeDescriptor::FixedLocation64 {
                                   is_writable: true,
                                   base_address: ((bar.get_address() as u64 & 0xFFFFFFF0) + ((nar.get_address() as u64 & 0xFFFFFFFF) << 32)),
                                   range_length: length + nlength
                                }))
                            } else {
                                let length = (!(barl & 0xFFFFFFF0) + 1) as u32;
                                Some(Resource::MemoryRange(MemoryRangeDescriptor::FixedLocation {
                                   is_writable: true,
                                   base_address: (bar.get_address() & 0xFFFFFFF0) ,
                                   range_length: length 
                                }))
                            }
                        }
                    } else {
                        None
                    };

                    if let Some(res) = res {
                        resources.push(res);
                    }
                    // let issome = res.is_some();
                    // ress[res_index] = res;
                    // if issome {
                    //     res_index += 1;
                    // }

                    i += 1;
                }
        };

        match ptype {
            0 => devs(6),
            1 => devs(2),
            _ => kprintln!("Unknown pci header type! ({})", ptype)
        }
       resources 
    }

    fn check_function<F: Fn(u16, u8, u8, u8)>(
        &self,
        segment: u16,
        bus: u8,
        device: u8,
        function: u8,
        callback: F,
    ) {
        callback(segment, bus, device, function);
    }

    fn check_device<F: Fn(u16, u8, u8, u8) + Copy>(
        &self,
        segment: u16,
        bus: u8,
        device: u8,
        callback: F,
    ) {
        let vendor = self.read_u16(segment, bus, device, 0, PCI::VENDOR_ID);
        if vendor == 0xFFFF {
            return;
        }

        self.check_function(segment, bus, device, 0, callback);

        let header_type = self.read_u8(segment, bus, device, 0, PCI::TYPE);
        if header_type & 0x80 != 0 {
            for function in 1..8 {
                let vendor = self.read_u16(segment, bus, device, function, PCI::VENDOR_ID);
                if vendor != 0xFFFF {
                    self.check_function(segment, bus, device, function, callback);
                }
            }
        }
    }

    fn traverse_devices<F: Fn(u16, u8, u8, u8) + Copy>(&self, callback: F) {
        let iter = self.mcfg.unwrap().iter();
        for entry in iter.enumerate() {
            for bus in 0..=255 {
                for device in 0..32 {
                    self.check_device(entry.0 as _, bus, device, callback);
                }
            }
        }
    }

    fn form_address<T>(address: u64, bus: u8, device: u8, function: u8, offset: u16) -> *const T {
        let address = address
            + ((bus as u64) << 20
                | (device as u64) << 15
                | (function as u64) << 12
                | (offset as u64) & 0xFFF);
        address as *const T
    }

    fn form_address_mut<T>(address: u64, bus: u8, device: u8, function: u8, offset: u16) -> *mut T {
        let address = address
            + ((bus as u64) << 20
                | (device as u64) << 15
                | (function as u64) << 12
                | (offset as u64) & 0xFFF);
        address as *mut T
    }

    pub fn read_u8(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u8 {
        let seg = &self.mcfg.unwrap()[segment];
        let address = PCI::form_address(seg.address, bus - seg.bus_start, device, function, offset);
        unsafe { core::ptr::read_volatile(address) }
    }

    pub fn read_u16(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u16 {
        let seg = &self.mcfg.unwrap()[segment];
        let address = PCI::form_address(seg.address, bus - seg.bus_start, device, function, offset);
        unsafe { core::ptr::read_volatile(address) }
    }

    pub fn read_u32(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u32 {
        let seg = &self.mcfg.unwrap()[segment];
        let address = PCI::form_address(seg.address, bus - seg.bus_start, device, function, offset);
        unsafe { core::ptr::read_volatile(address) }
    }

    pub fn write_u8(
        &mut self,
        segment: u16,
        bus: u8,
        device: u8,
        function: u8,
        offset: u16,
        value: u8,
    ) {
        let seg = &self.mcfg.unwrap()[segment];
        let address =
            PCI::form_address_mut(seg.address, bus - seg.bus_start, device, function, offset);
        unsafe { core::ptr::write_volatile(address, value) }
    }

    pub fn write_u16(
        &mut self,
        segment: u16,
        bus: u8,
        device: u8,
        function: u8,
        offset: u16,
        value: u16,
    ) {
        let seg = &self.mcfg.unwrap()[segment];
        let address =
            PCI::form_address_mut(seg.address, bus - seg.bus_start, device, function, offset);
        unsafe { core::ptr::write_volatile(address, value) }
    }

    pub fn write_u32(
        &mut self,
        segment: u16,
        bus: u8,
        device: u8,
        function: u8,
        offset: u16,
        value: u32,
    ) {
        let seg = &self.mcfg.unwrap()[segment];
        let address =
            PCI::form_address_mut(seg.address, bus - seg.bus_start, device, function, offset);
        unsafe { core::ptr::write_volatile(address, value) }
    }
}

pub fn class_str(class: u8, subclass: u8, prog_if: u8) -> &'static str {
    match class {
        0x00 => match subclass {
            0x00 => "Non-VGA-Compatible Unclassified Device",
            0x01 => "VGA-Compatible Unclassified Device",
            _ => "Unclassified",
        },
        0x01 => match subclass {
            0x00 => "SCSI Bus Controller",
            0x01 => "IDE Controller",
            0x02 => "Floppy Disk Controller",
            0x03 => "IPI Bus Controller",
            0x04 => "RAID Controller",
            0x05 => "ATA Controller",
            0x06 => "Serial ATA Controller",
            0x07 => "Serial Attached SCSI Controller",
            0x08 => "Non-Volatile Memory Controller",
            0x80 => "Other",
            _ => "Mass Storage Controller",
        },
        0x02 => match subclass {
            0x00 => "Ethernet Controller",
            0x01 => "Token Ring Controller",
            0x02 => "FDDI Controller",
            0x03 => "ATM Controller",
            0x04 => "ISDN Controller",
            0x05 => "WorldFip Controller",
            0x06 => "PICMG 2.14 Multi Computing Controller",
            0x07 => "Infiniband Controller",
            0x08 => "Fabric Controller",
            0x80 => "Other",
            _ => "Network Controller",
        },
        0x03 => match subclass {
            0x00 => "VGA Compatible Controller",
            0x01 => "XGA Controller",
            0x02 => "3D Controller (Not VGA-Compatible)",
            0x80 => "Other",
            _ => "Network Controller",
        },
        0x04 => match subclass {
            0x00 => "Multimedia Video Controller",
            0x01 => "Multimedia Audio Controller",
            0x02 => "Computer Telephony Device",
            0x03 => "Audio Device",
            0x80 => "Other",
            _ => "Network Controller",
        },
        0x05 => match subclass {
            0x00 => "RAM Controller",
            0x01 => "Flash Controller",
            0x80 => "Other",
            _ => "Network Controller",
        },
        0x06 => match subclass {
            0x00 => "Host Bridge",
            0x01 => "ISA Bridge",
            0x02 => "EISA Bridge",
            0x03 => "MCA Bridge",
            0x04 => "PCI-to-PCI Bridge",
            0x05 => "PCMCIA Bridge",
            0x06 => "NuBus Bridge",
            0x07 => "CardBus Bridge",
            0x08 => "RACEway Bridge",
            0x09 => "PCI-to-PCI Bridge",
            0x0A => "InfiniBand-to-PCI Host Bridge",
            0x80 => "Other",
            _ => "Network Controller",
        },
        0x07 => match subclass {
            0x00 => "Serial Controller",
            0x01 => "Parallel Controller",
            0x02 => "Multiport Serial Controller",
            0x03 => "Modem",
            0x04 => "IEEE 488.1/2 (GPIB) Controller",
            0x05 => "Smart Card Controller",
            0x80 => "Other",
            _ => "Network Controller",
        },
        0x08 => match subclass {
            0x00 => "PIC",
            0x01 => "DMA Controller",
            0x02 => "Timer",
            0x03 => "RTC Controller",
            0x04 => "PCI Hot-Plug Controller",
            0x05 => "SD Host controller",
            0x06 => "IOMMU",
            0x80 => "Other",
            _ => "Network Controller",
        },
        0x09 => match subclass {
            0x00 => "Keyboard Controller",
            0x01 => "Digitizer Pen",
            0x02 => "Mouse Controller",
            0x03 => "Scanner Controller",
            0x04 => "Gameport Controller",
            0x80 => "Other",
            _ => "Network Controller",
        },
        0x0A => match subclass {
            0x00 => "Generic",
            0x80 => "Other",
            _ => "Docking Station",
        },
        0x0B => match subclass {
            0x00 => "386",
            0x01 => "486",
            0x02 => "Pentium",
            0x03 => "Pentium Pro",
            0x04 => "Alpha",
            0x05 => "PowerPC",
            0x06 => "MIPS",
            0x07 => "Co-Processor",
            0x80 => "Other",
            _ => "Docking Station",
        },
        0x0C => match subclass {
            0x00 => "FireWire (IEEE 1394) Controller",
            0x01 => "ACCESS Bus Controller",
            0x02 => "SSA",
            0x03 => "USB Controller",
            0x04 => "Fibre Channel",
            0x05 => "SMBus Controller",
            0x06 => "InfiniBand Controller",
            0x07 => "IPMI Interface",
            0x08 => "SERCOS Interface (IEC 61491)",
            0x09 => "CANbus Controller",
            0x80 => "Other",
            _ => "Docking Station",
        },
        0x0D => match subclass {
            0x00 => "iRDA Compatible Controller",
            0x01 => "Consumer IR Controller",
            0x10 => "RF Controller",
            0x11 => "Bluetooth Controller",
            0x12 => "Broadband Controller",
            0x20 => "Ethernet Controller (802.1a)",
            0x21 => "Ethernet Controller (802.1b)",
            0x80 => "Other",
            _ => "Docking Station",
        },
        _ => "Unknown",
    }
}
