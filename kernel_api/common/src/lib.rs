#![no_std]
#![feature(crate_visibility_modifier)]
#![feature(alloc_error_handler)]

pub mod efi;
pub mod elf;
#[macro_use]
pub mod util;
pub mod serial;
pub mod gdt;
pub mod mem;
pub mod allocator;
pub mod kernel_process;
mod linked_list_allocator;

pub use x86_64;

extern crate alloc;

pub struct KernelParameters<'a> {
    pub memory_map: &'a [efi::MemoryDescriptor],
    pub boot_image: &'a boot_fs::BootImageFS<'a>,
}