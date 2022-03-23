use super::xsdt::{Header, XSDTEntry};

#[repr(u8)]
pub enum AddressSpace {
    SystemMemory = 0,
    SystemIO,
    PCIConfig,
    Embedded,
    SMB,
    CMOS,
    PCIBAR,
    IPMI,
    GPIO,
    Serial,
    CommChannel
}

#[repr(u8)]
pub enum AccessSize {
    Byte = 1,
    Word,
    Double,
    Quad,
}

#[repr(C, packed)]
pub struct GenericAddress {
    crate address_space: AddressSpace,
    crate bit_width: u8,
    crate bit_offset: u8,
    crate access_size: AccessSize,
    crate address: u64,
}

#[repr(C, packed)]
pub struct FADT {
    crate header: Header,
    _firmware_control:  u32,
    _dsdt: u32,

    _reserved: u8,

    crate power_management_profile: u8,
    crate sci_interrupt: u16,
    crate smi_command_port: u32,
    crate acpi_enable: u8,
    crate acpi_disable: u8,
    crate s4bios_req: u8,
    crate pstate_control: u8,

    _reserved1: [u32; 8],
    _reserved2: [u8; 7],

    crate cstate_control: u8,
    crate worst_c2_latency: u16,
    crate worst_c3_latency: u16,
    crate flush_size: u16,
    crate flush_stride: u16,
    crate duty_offset: u8,
    crate duty_width: u8,
    crate day_alarm: u8,
    crate month_alarm: u8,
    crate century: u8,

    crate arch_flags: u16,

    _reserved3: u8,
    crate flags: u32,

    crate reset_reg: GenericAddress,
    crate reset_value: u8,

    _reserved4: [u8; 3],

    crate firmware_control: u64,
    crate dsdt: *const DSDT,

    crate pm1a_event_block: GenericAddress,
    crate pm1b_event_block: GenericAddress,
    crate pm1a_control_block: GenericAddress,
    crate pm1b_control_block: GenericAddress,
    crate pm2_control_block: GenericAddress,
    crate pm_timer_block: GenericAddress,
    crate gpe0_block: GenericAddress,
    crate gpe1_block: GenericAddress,
}

impl XSDTEntry for FADT {}

#[repr(C, packed)]
pub struct DSDT {
    crate header: Header,
    crate data: u8
}