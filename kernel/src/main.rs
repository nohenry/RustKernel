#![no_std]
#![no_main]
#![allow(non_snake_case)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![feature(crate_visibility_modifier)]
#![feature(arbitrary_enum_discriminant)]
#![feature(bench_black_box)]
#![feature(inline_const)]
#![allow(unconditional_panic)]

extern crate alloc;

mod acpi;
mod drivers;
mod interrupts;
mod process_manager;
mod syscall;

use core::arch::{asm, x86_64};
use core::panic::PanicInfo;

use boot_fs::BootImageFS;
use common::memory_regions::PAGE_TABLE_OFFSET;
use common::serial::SerialPort;
use macros::wchar;

use common::x86_64::registers::control::{Cr3, Cr3Flags};
use common::x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size2MiB,
    Size4KiB, Translate,
};
use common::x86_64::{PhysAddr, VirtAddr};
use common::{allocator, efi, elf, gdt, kprint, kprintln, mem, process, size_gb, KernelParameters};

use crate::drivers::pci;
use crate::process_manager::ManagedProcess;
use common::efi::{
    get_system_table, guid, FileHandle, FileInfo, FileProtocol, FILE_HIDDEN, FILE_MODE_READ,
    FILE_READ_ONLY, FILE_SYSTEM,
};

const BOOT_IMAGE: u64 = size_gb!(100);

#[no_mangle]
pub extern "C" fn _start(parameters: &'static mut KernelParameters) -> ! {
    // kprintln!("Kernel... {:p}", parameters.system_table);

    use core::fmt;
    let mut serial = SerialPort::from(0x3F8);
    fmt::write(&mut serial, format_args!("Kernel.. {:p}", parameters.system_table)).expect("Unable to print!");
    fmt::write(&mut serial, format_args!("\r\n")).expect("Unable to print!");

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
    let mut mapper = unsafe { mem::init(parameters.frame_allocator.clone(), PAGE_TABLE_OFFSET) };
    mem::allocator().lock().swap_map(parameters.memory_map);

    let mem_size = efi::get_mem_size(parameters.memory_map);
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
    mem::map_virt::<Size2MiB>(
        PhysAddr::new(parameters.boot_image.0),
        VirtAddr::new(BOOT_IMAGE),
        parameters.boot_image.1 as usize,
    )
    .unwrap();

    let pt = PhysFrame::<Size2MiB>::containing_address(PhysAddr::new(parameters.boot_image.0));
    let pt_end = PhysFrame::<Size2MiB>::containing_address(PhysAddr::new(
        parameters.boot_image.0 + parameters.boot_image.1,
    ));

    // let pt = PhysFrame::<Size2MiB>::containing_address(VirtAddr::new(BOOT_IMAGE));
    // let pt_end = PhysFrame::<Size2MiB>::containing_address(VirtAddr::new(
    //     parameters.boot_image.0 + parameters.boot_image.1,
    // ));
    // let pgs = Page::<Size2MiB>::range_inclusive(pt, pt_end);

    for frame in PhysFrame::<Size2MiB>::range_inclusive(pt, pt_end) {
        let page = Page::containing_address(VirtAddr::new(
            (frame.start_address().as_u64() - pt.start_address().as_u64()) + BOOT_IMAGE,
        ));
        kprintln!(
            "Mapping {:x} - {:x}",
            frame.start_address().as_u64(),
            page.start_address().as_u64()
        );
        unsafe {
            <OffsetPageTable as Mapper<Size2MiB>>::map_to(
                &mut mem::active_offset_page_table(PAGE_TABLE_OFFSET),
                page,
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                mem::allocator().get_mut(),
            )
        };
    }

    let ptr = BOOT_IMAGE + parameters.boot_image.0 - pt.start_address().as_u64();
    let ptr = ptr as *const u8;
    for i in 0..32 {
        kprint!("{:02X} ", unsafe { *ptr.offset(i) });
    }
    let file_data =
        unsafe { core::slice::from_raw_parts(ptr as *const u8, parameters.boot_image.1 as usize) };

    let image = BootImageFS::new(file_data);
    process::set_syscall_sp();

    kprintln!("Boot Image: ");
    for file in image.files() {
        kprintln!("  {}", file.name());
    }
    // let mut image: Option<elf::ElfFile> = None;
    let mut files = image.files();

    let kernel = files.next().unwrap();
    let kernel_exec_file = elf::ElfFile::new(image.file_data(kernel));

    let driver = files.next().unwrap();
    let driver_exec_file = elf::ElfFile::new(image.file_data(driver));
    let ddate = driver_exec_file.data;

    let new_process =
        ManagedProcess::new_kernel_process(&driver_exec_file, &kernel_exec_file, 0, 0, mem_size);

    // unsafe {
    //     processes::jump_usermode(&mapper, &new_process);
    // }

    process_manager::init();

    common::x86_64::instructions::interrupts::enable();

    kprintln!("Done!");
    loop {}
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    kprintln!("PANIC! {}\n", _info);
    loop {}
}
