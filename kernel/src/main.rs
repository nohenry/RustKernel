#![no_std]
#![no_main]
#![allow(non_snake_case)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![feature(crate_visibility_modifier)]
#![feature(arbitrary_enum_discriminant)]
#![allow(unconditional_panic)]

extern crate alloc;

mod acpi;
mod drivers;
mod interrupts;
mod processes;

use core::arch::asm;
use core::panic::PanicInfo;

use macros::wchar;

use common::x86_64::registers::control::{Cr3, Cr3Flags};
use common::x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    Translate,
};
use common::x86_64::{PhysAddr, VirtAddr};
use common::{allocator, elf, gdt, kprintln, mem, KernelParameters};

use crate::drivers::pci;
use crate::processes::{test_process, Process};
use common::efi::{
    get_system_table, guid, FileHandle, FileInfo, FileProtocol, FILE_HIDDEN, FILE_MODE_READ,
    FILE_READ_ONLY, FILE_SYSTEM,
};

#[no_mangle]
pub extern "C" fn _start(parameters: &KernelParameters) -> ! {
    kprintln!("Kernel...");
    let wait = false;
    while wait {
        unsafe { asm!("pause") }
    }
    acpi::init();

    // Setup interrupts
    interrupts::init();

    let mut mapper = unsafe { mem::init() };
    let mut frame_allocator = mem::PageTableFrameAllocator::new(parameters.memory_map);

    let mut npt = mapper.level_4_table().clone();
    let mut mapper = unsafe { OffsetPageTable::new(&mut npt, VirtAddr::new(0)) };
    let table: *mut PageTable = mapper.level_4_table();

    unsafe {
        Cr3::write(
            PhysFrame::from_start_address(PhysAddr::new(table as u64))
                .expect("Unable to switch page table!"),
            Cr3Flags::empty(),
        );
        mem::KERNEL_MAP = table as u64;
    }

    allocator::init_heap(&mut mapper, &mut frame_allocator, false).expect("Unable to create heap!");

    unsafe {
        mapper.identity_map(
            PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(0xFEE00000)),
            PageTableFlags::WRITABLE,
            &mut frame_allocator,
        );
    }

    pci::init();
    acpi::aml::init();
    //pci::gather_devices();

    // interrupts::enable_apic();

    kprintln!("Boot Image: ");
    let mut image: Option<elf::ElfFile> = None;
    for file in parameters.boot_image.files() {
        kprintln!("  {}", file.name());
        let exec_file = elf::ElfFile::new(parameters.boot_image.file_data(file));
        image = Some(exec_file);
    }

    processes::set_syscall_sp();


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
