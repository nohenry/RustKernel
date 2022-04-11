#![no_std]
#![no_main]
#![allow(non_snake_case)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![feature(crate_visibility_modifier)]
#![feature(arbitrary_enum_discriminant)]
#![feature(bench_black_box)]
#![allow(unconditional_panic)]

extern crate alloc;

mod acpi;
mod drivers;
mod interrupts;
mod syscall;
mod process_manager;

use core::arch::asm;
use core::panic::PanicInfo;

use macros::wchar;

use common::x86_64::registers::control::{Cr3, Cr3Flags};
use common::x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    Translate,
};
use common::x86_64::{PhysAddr, VirtAddr};
use common::{allocator, efi, elf, gdt, kprintln, mem, KernelParameters, process};

use crate::drivers::pci;
use common::efi::{
    get_system_table, guid, FileHandle, FileInfo, FileProtocol, FILE_HIDDEN, FILE_MODE_READ,
    FILE_READ_ONLY, FILE_SYSTEM,
};

#[no_mangle]
pub extern "C" fn _start(parameters: &'static mut KernelParameters) -> ! {
    kprintln!("Kernel... {:p}", parameters.system_table);
    allocator::init_heap(&parameters.heap);

    unsafe {
        // Set the static system table reference
        efi::register_global_system_table(parameters.system_table).unwrap();
    }

    let wait = false;
    while core::convert::identity(wait) {
        unsafe { asm!("pause") }
    }

    // let frame_allocator = mem::PageTableFrameAllocator::new(parameters.memory_map);
    let mut mapper =
        unsafe { mem::init(parameters.frame_allocator.clone(), mem::PAGE_TABLE_OFFSET) };
    mem::allocator().lock().swap_map(parameters.memory_map);

    // unsafe {
    //     mem::KERNEL_MAP = table as u64;
    // }

    // efi::print_memory_map(parameters.memory_map);
    // allocator::init_heap_new(&mut mapper, &mut frame_allocator, parameters.heap_top, false).expect("Unable to create heap!");

    // acpi::init(parameters.memory_map);
    acpi::init(parameters.memory_map);

    // Setup interrupts
    interrupts::init();

    pci::init();
    acpi::aml::init();
    //pci::gather_devices();

    // interrupts::enable_apic();

    // kprintln!("Boot Image: ");
    // let mut image: Option<elf::ElfFile> = None;
    // for file in parameters.boot_image.files() {
    //     kprintln!("  {}", file.name());
    //     let exec_file = elf::ElfFile::new(parameters.boot_image.file_data(file));
    //     image = Some(exec_file);
    // }

    process::set_syscall_sp();

    // let new_process = Process::from_elf(&image.unwrap(),  &mut mapper, &mut frame_allocator);

    // unsafe {
    //     processes::jump_usermode(&mapper, &new_process);
    // }

    kprintln!("Done!");
    loop {}
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    kprintln!("PANIC! {}\n", _info);
    loop {}
}
