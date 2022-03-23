use core::{marker::PhantomData, ops::Index};

use super::xsdt::{Header, XSDTEntry};

#[repr(C, packed)]
pub struct MCFG<'a> {
    crate header: Header,
    _reserved: u64,

    entries: Entry,
    pd: PhantomData<&'a ()>,
}

impl MCFG<'_> {
    pub fn iter(&self) -> MCFGIterator {
        MCFGIterator::new(self, self.len())
    }

    pub fn len(&self) -> usize {
        (self.header.length as usize - core::mem::size_of::<Header>())
            / core::mem::size_of::<Entry>()
    }
}

impl<'a> Index<u16> for MCFG<'a> {
    type Output = Entry;

    fn index(&self, index: u16) -> &Self::Output {
        let address: *const Entry = &self.entries;
        unsafe { &*address.offset(index as _) }
    }
}

impl XSDTEntry for MCFG<'_> {}

#[repr(C, packed)]
pub struct Entry {
    crate address: u64,
    crate segment: u16,
    crate bus_start: u8,
    crate bus_end: u8,
    _reserved: u32,
}

pub struct MCFGIterator<'a> {
    mcfg: &'a MCFG<'a>,
    size: usize,
    index: usize,
}

impl<'a> MCFGIterator<'a> {
    fn new(xsdt: &'a MCFG, size: usize) -> MCFGIterator<'a> {
        MCFGIterator {
            mcfg: xsdt,
            size,
            index: 0,
        }
    }
}

impl<'a> Iterator for MCFGIterator<'a> {
    type Item = &'a Entry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            let address: *const Entry = &self.mcfg.entries;
            let entry = unsafe { &*address.offset(self.index as _) };
            self.index += 1;

            Some(entry)
        } else {
            None
        }
    }
}
