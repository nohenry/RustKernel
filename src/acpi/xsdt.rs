#[repr(C, packed)]
pub struct Header {
    crate signature: [u8; 4],
    crate length: u32,
    crate revision: u8,
    crate checksum: u8,
    crate oem: [u8; 6],
    crate oem_table: [u8; 8],
    crate oem_revision: u32,
    crate creator: u32,
    crate creator_revision: u32,
}

impl Header {
    pub fn signature(&self) -> &str {
        core::str::from_utf8(&self.signature).unwrap()
    }

    crate fn get_entry<T: XSDTEntry>(&self) -> &T {
        let ptr = self as *const Self;
        unsafe { &*(ptr as *const T) }
    }
}

crate trait XSDTEntry {}

#[repr(C, packed)]
pub struct XSDT {
    crate header: Header,
    crate tables: [*const Header],
}

impl XSDT {
    pub fn size(&self) -> usize {
        (self.header.length as usize - core::mem::size_of::<Header>()) / 8
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

impl<'a> XSDTIterator<'a> {
    fn new(xsdt: &'a XSDT, size: usize) -> XSDTIterator {
        XSDTIterator {
            xsdt,
            size,
            index: 0,
        }
    }
}

impl<'a> Iterator for XSDTIterator<'a> {
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
