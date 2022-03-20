
#[repr(C, packed)]
pub struct RSDP {
    signature: [char; 8],
    checksum: u8,
    oem: [char; 6],
    revision: u8,
    rsdt_address: u32,
    length: u32,
    xsdt: *const XSDT,
    ext_checksum: u8,
}

#[repr(C, packed)]
pub struct XSDT {

}