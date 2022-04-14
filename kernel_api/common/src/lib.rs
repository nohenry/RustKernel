#![no_std]
#![feature(crate_visibility_modifier)]
#![feature(alloc_error_handler)]
#![feature(abi_efiapi)]

pub mod efi;
pub mod elf;
#[macro_use]
pub mod util;
pub mod serial;
pub mod gdt;
pub mod mem;
pub mod allocator;
pub mod process;
pub mod memory_regions;
mod linked_list_allocator;

use core::fmt::Debug;

use efi::SystemTable;
use mem::PageTableFrameAllocator;
pub use x86_64;
use x86_64::structures::paging::PageTable;

extern crate alloc;

pub struct KernelParameters<'a> {
    pub memory_map: &'a [efi::MemoryDescriptor],
    // Physical address of boot image
    pub boot_image: (u64, u64),
    pub frame_allocator: PageTableFrameAllocator<'a>,
    pub system_table: *mut SystemTable,
    // pub heap_top: usize,
    pub heap: linked_list_allocator::Heap,
    // pub page_table: PageTable,
}

impl Debug for KernelParameters<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KernelParameters").field("boot_image", &self.boot_image).finish()
    }
}