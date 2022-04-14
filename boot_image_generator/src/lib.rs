#![no_std]

use core::marker::PhantomData;
use core::iter::Iterator;
use core::option::Option::{self, None, Some};


#[repr(C, packed)]
pub struct FileHeader {
    pub magic: u16,
    pub name: [u8; 16],
    pub file_offset: u32,
    pub file_length: u32,
}

impl FileHeader {
    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name).expect("Unable to get string from file name!")
    }
}

pub struct FileIterator<'a> {
    base: *const FileHeader,
    index: usize,
    size: usize,
    pd: PhantomData<&'a ()>
}

impl <'a> Iterator for FileIterator<'a> {
    type Item = &'a FileHeader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            let value = unsafe { &*self.base.offset(self.index as _) };
            self.index += 1;
            Some(value)
        } else {
            None
        }
    }

}

#[derive(Debug)]
pub struct BootImageFS<'a> {
    data: &'a [u8],

}

impl BootImageFS<'_> {
    pub fn new(data: &[u8]) -> BootImageFS {
        BootImageFS {
            data
        }
    }

    pub fn files(&self) -> FileIterator {
        let len_buf = [self.data[0], self.data[1]];
        let len = u16::from_ne_bytes(len_buf);

        FileIterator { base: unsafe { (self.data as *const [u8] as  *const u8).offset(2) as *const FileHeader }, index: 0, size: len as _, pd: PhantomData }
    }

    pub fn file_data(&self, header: &FileHeader) -> &[u8] {
        &self.data[(header.file_offset as usize)..(header.file_offset+header.file_length) as usize] 
    }

    pub fn virtual_address(&self) -> u64 {
        &self.data[0] as *const u8 as _ 
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}


