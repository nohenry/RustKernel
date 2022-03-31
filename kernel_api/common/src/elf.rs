use core::{marker::PhantomData, fmt::{Formatter, write, Error, Debug}};


#[repr(u16)]
#[derive(Debug)]
pub enum Machine {
    None = 0,
    M32,
    SPARC,
    I386,
    M68K,
    M88K,
    I860 = 7,
    MIPS,
    PowerPC = 0x14,
    ARM = 0x28,
    SuperH = 0x2A,
    IA64 = 0x32,
    Amd64 = 0x3E,
    AArch64 = 0xB7,
    RISCV = 0xF3,
}

#[repr(u16)]
#[derive(Debug)]
pub enum FileType {
    None = 0,
    Relocatable,
    Executable,
    SharedObject,
    Core
}

#[repr(u8)]
#[derive(Debug)]
pub enum BitSize {
    X32 = 1,
    X64 = 2,
}

#[repr(u8)]
#[derive(Debug)]
pub enum Endianess {
    Little = 1,
    Big = 2,
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct Header {
    ident: u32,
    bits: BitSize,
    endianess: Endianess,
    abi: u8,
    header_version: u8,
    _reserved: u64,
    file_type: FileType,
    machine: Machine,
    version: u32,
    pub entry: u64,
    prg_header_tbl: u64,
    sec_header_tbl: u64,
    flags: u32,
    header_size: u16,
    prg_entry_size: u16,
    prg_entry_count: u16,
    sec_entry_size: u16,
    sec_entry_count: u16,
    sec_str_index: u16,
}

impl Header {
    pub fn program_header_table(&self) -> *const ProgramHeader {
        unsafe { (self as *const Header as *const u8).offset(0 as _) as *const ProgramHeader } 
    }
}

pub struct ElfFile<'a> {
    data: &'a [u8]
}

impl <'a> ElfFile<'a> {
    pub fn new(data: &'a [u8]) -> ElfFile {
        ElfFile { data }
    }

    pub fn header(&self) -> &Header {
        unsafe {
            &*(&self.data[0] as *const u8 as *const _)
        }
    }

    pub fn progam_headers(&self) -> ProgramHeaderIterator {
        let header = self.header();
        let base = unsafe { &self.data[header.prg_header_tbl as usize] as *const u8 as *const ProgramHeader };
        assert!(header.prg_entry_size as usize == core::mem::size_of::<ProgramHeader>(), "Sizes aren't equal {} == {}", header.prg_entry_size, core::mem::size_of::<ProgramHeader>());
        ProgramHeaderIterator {
            base,
            index: 0,
            size: header.prg_entry_count as _,
            pd: PhantomData
        }
    }

    pub fn segment(&self, header: &ProgramHeader) -> Option<&[u8]> {
        if header.segment_file_size > 0 {
            Some(&self.data[(header.offset as usize)..(header.offset+header.segment_file_size) as usize])
        } else {
            None
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum SegmentType {
    Null = 0,
    Load,
    Dynamic,
    Interpret,
    Note,
    Reserved,
    ProgramHeader
}

impl Debug for SegmentType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match *self as u32 {
            0 => write(f, format_args!("Null")),
            1 => write(f, format_args!("Load")),
            2 => write(f, format_args!("Dynamic")),
            3 => write(f, format_args!("Interpret")),
            4 => write(f, format_args!("Note")),
            5 => write(f, format_args!("Reserved")),
            6 => write(f, format_args!("ProgramHeader")),
            0x70000000.. => write(f, format_args!("Lo Proc")),
            0x7FFFFFFF.. => write(f, format_args!("High Proc")),
            _ => write!(f, "Unknown Segment Type"),
        }
    }
}

#[repr(u32)]
#[derive(Debug)]
pub enum ProgramHeaderFlags {
    Executable = 1,
    Writable,
    Readable = 4,
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct ProgramHeader {
    pub segement_type: SegmentType,
    pub flags: u32,
    pub offset: u64,
    pub virtual_address: u64,
    _reserved: u64,
    pub segment_file_size: u64,
    pub segment_mem_size: u64,
    _align: u64
}

pub struct ProgramHeaderIterator<'a> {
    base: *const ProgramHeader,
    index: usize,
    size: usize,
    pd: PhantomData<&'a ()>,
}

impl <'a> Iterator for ProgramHeaderIterator<'a> {
    type Item = &'a ProgramHeader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            let res = unsafe {
                &*self.base.offset(self.index as _)
            };
            self.index += 1;
            Some(res)
        } else {
            None
        }
    }
}
