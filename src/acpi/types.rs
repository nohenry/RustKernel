use super::xsdt::XSDT;

#[repr(C, packed)]
pub struct RSDP {
    crate signature: [u8; 8],
    crate checksum: u8,
    crate oem: [u8; 6],
    crate revision: u8,
    crate rsdt_address: u32,
    crate length: u32,
    crate xsdt: *const XSDT,
    crate ext_checksum: u8,
    crate reserved: [u8; 3],
}

pub enum Signature {
    MADT,
    FADT,
    MCFG,
}

impl Signature {
    crate fn as_bytes(&self) -> &[u8] {
        match self {
            Signature::MADT => "APIC".as_bytes(),
            Signature::FADT => "FACP".as_bytes(),
            Signature::MCFG => "MCFG".as_bytes(),
        }
    }
}