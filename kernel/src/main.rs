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

mod drivers;
mod efi;
#[macro_use]
mod util;
mod acpi;
mod allocator;
mod gdt;
mod interrupts;
mod linked_list_allocator;
mod mem;
mod processes;

use core::arch::asm;
use core::panic::PanicInfo;

use macros::wchar;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::{
    Mapper, OffsetPageTable, PageTable, PageTableFlags, PhysFrame, Size4KiB, Translate,
};
use x86_64::{PhysAddr, VirtAddr};

use crate::drivers::pci;
use crate::efi::{
    FileHandle, FileProtocol, FILE_HIDDEN, FILE_MODE_READ, FILE_READ_ONLY, FILE_SYSTEM,
};
use crate::processes::{test_process, Process};

#[no_mangle]
extern "C" fn efi_main(image_handle: efi::Handle, system_table: *mut efi::SystemTable) {
    unsafe {
        // Set the static system table reference
        efi::register_global_system_table(system_table).unwrap();
    }

    // wchar!("Potato");
    let base = efi::get_image_base(image_handle);
    kprintln!("Entry: {:x}", base);
    kprintln!("System Init");

    let volume = unsafe { &*efi::io_volume(image_handle) };
    let mut fileio: *const FileProtocol = core::ptr::null();
    let res = (volume.open_volume)(volume as _, &mut fileio);
    if res != 0 {
        kprintln!("An error occured! {:x} OpenVolume(SFSP)", res);
    }
    let fileio = unsafe { &*fileio };
    let mut newfileio: *const FileProtocol = core::ptr::null();
    let res = (fileio.open)(
        fileio as _,
        &mut newfileio,
        wchar!("boot_image.bin") as *const _,
        FILE_MODE_READ,
        FILE_READ_ONLY | FILE_HIDDEN | FILE_SYSTEM,
    );
    if res != 0 {
        kprintln!("An error occured! {:x} OPEN(SFSP)", res);
    }

    let wait = true;
    while wait {
        unsafe { asm!("pause") }
    }
    acpi::init();

    // Iterate memorymap and exit boot services
    let memory_map = efi::get_memory_map(image_handle);

    // Setup global descriptor table :P
    gdt::init();

    // Setup interrupts
    interrupts::init();

    let mut mapper = unsafe { mem::init() };
    let mut frame_allocator = mem::PageTableFrameAllocator::new(memory_map);

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
    pci::gather_devices();

    // let i = 5 / 0;
    // let addresses = [processes::test_process as u64]; // same as before

    // for &address in &addresses {
    //     let virt = VirtAddr::new(address);
    //     let res = mapper.translate(virt);
    //     match res {
    //         TranslateResult::Mapped { frame, flags, .. } => {
    //             kprintln!("Frame: {:#x?} {:?}", frame, flags);
    //         }
    //         _ => (),
    //     }
    // }
    // interrupts::enable_apic();

    // processes::set_syscall_sp();

    // let new_process = Process::new(test_process, &mut frame_allocator);

    // unsafe {
    //     processes::jump_usermode(&mapper, &new_process);
    // }

    kprintln!("Done!");

    loop {}
    // panic!("Kernel Finished");
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    kprintln!("PANIC! {}\n", _info);
    loop {}
}
