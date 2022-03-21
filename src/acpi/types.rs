

#[repr(C, packed)]
pub struct RSDP {
    crate signature: [char; 8],
    crate checksum: u8,
    crate oem: [char; 6],
    crate revision: u8,
    crate rsdt_address: u32,
    crate length: u32,
    crate xsdt: *const XSDT,
    crate ext_checksum: u8,
}

#[repr(C, packed)]
pub struct Header {
    crate signature: [char; 4],
    crate length: u32,
    crate revision: u8,
    crate checksum: u8,
    crate oem: [char; 6],
    crate oem_table: [char; 8],
    crate oem_revision: u32,
    crate creator: u32,
    crate creator_revision: u32,
}

impl Header {
    pub fn signature(&self) -> &str {
        let pp = &self.signature;
        kprintln!("{}", pp.len());
        ""
        // core::str::from_utf8(unsafe {&*(self.signature.as_ref() as *const _ as *const [u8])}).unwrap()
    }
}

#[repr(C, packed)]
pub struct XSDT {
    crate header: Header,
    crate tables: [*const Header]
}

impl XSDT {
    pub fn size(&self) -> usize {
        (self.header.length as usize - core::mem::size_of::<Header>()) / core::mem::size_of::<*const Header>()
    }

    pub fn iter(&self) -> XSDTIterator {
        XSDTIterator::new(self, self.size())
    }
}

pub struct XSDTIterator<'a> {
    xsdt: &'a XSDT,
    size: usize,
    index: usize,
}

impl <'a> XSDTIterator<'a> {
    fn new(xsdt: &'a XSDT, size: usize) -> XSDTIterator{
        XSDTIterator{
            xsdt,
            size,
            index: 0
        }
    }
}

impl <'a>Iterator for XSDTIterator<'a>{
    type Item = &'a Header;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            let ret = Some(unsafe { &*self.xsdt.tables[self.index] });
            self.index += 1;
            ret
        } else {
            None
        }
    }
}