#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(crate_visibility_modifier)]
#![feature(arbitrary_enum_discriminant)]
#![allow(unconditional_panic)]

extern crate alloc;

use core::panic::PanicInfo;
use core::{alloc::Layout, arch::asm};

use macros::wchar;

use common::{
    allocator,
    efi::{
        self, get_system_table, guid, FileHandle, FileInfo, FileProtocol, FILE_HIDDEN,
        FILE_MODE_READ, FILE_READ_ONLY, FILE_SYSTEM,
    },
    elf, gdt,
    kernel_process::KernelProcess,
    kprintln, mem, KernelParameters,
};

use common::x86_64::registers::control::{Cr3, Cr3Flags};
use common::x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    Translate,
};
use common::x86_64::{PhysAddr, VirtAddr};

#[no_mangle]
extern "C" fn efi_main(image_handle: efi::Handle, system_table: *mut efi::SystemTable) {
    unsafe {
        // Set the static system table reference
        efi::register_global_system_table(system_table).unwrap();
    }

    //let base = efi::get_image_base(image_handle);
    //kprintln!("Entry: {:x}", base);

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
        wchar!("efi\\boot\\btimg.bin") as *const _,
        FILE_MODE_READ,
        FILE_READ_ONLY,
    );
    if res != 0 {
        kprintln!("An error occured! {:x} OPEN(SFSP)", res);
    }

    let mut file_info: FileInfo = unsafe { core::mem::zeroed() };
    let buffer: *mut FileInfo = &mut file_info;
    let mut size = core::mem::size_of::<FileInfo>();

    let res = (fileio.get_info)(
        newfileio,
        &guid::FILE_INFO,
        &mut size,
        buffer as *mut u8 as *mut (),
    );

    if res != 0 {
        kprintln!("An error occured! {:x} GETINFO(SFSP)", res);
    }
    kprintln!("{:?} {}", file_info, size);

    let mut file_data: *mut u8 = core::ptr::null_mut();
    let efi_table = get_system_table();
    let res = efi_table
        .boot_services()
        .allocate_pool(file_info.file_size + 1, &mut file_data);

    if res != 0 {
        kprintln!("An error occured! {:x} ALLOCATEPOOL(SFSP)", res);
    }

    let copy_file_data = unsafe {
        core::slice::from_raw_parts_mut((file_data as *mut _) as *mut u8, file_info.file_size + 1)
    };

    efi::read_fixed(unsafe { &*newfileio }, 0, file_info.file_size, copy_file_data);

    if res != 0 {
        kprintln!("An error occured! {:x} FREEPOOL(SFSP)", res);
    }


    // let mut copy_top = 0u64;
    // unsafe {
    //     asm!("mov {}, rsp", out(reg) copy_top);
    // }
    // Iterate memorymap and exit boot services
    let memory_map = efi::get_memory_map(image_handle);

    // Setup global descriptor table :P
    gdt::init();

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

    let layout = Layout::from_size_align( file_info.file_size, 1).unwrap();
    kprintln!("{:?}", &layout);
    let img_data = unsafe { alloc::alloc::alloc(layout) };
    kprintln!("{:p}", img_data);
    let file_data = unsafe {
        common::util::memcpy(img_data, file_data, file_info.file_size);
        core::slice::from_raw_parts_mut(img_data, file_info.file_size)
    };
    kprintln!("Here");

    // let res = efi_table.boot_services().free_pool(copy_file_data);
    kprintln!("Potato");

    let boot_image = boot_fs::BootImageFS::new(file_data);

    kprintln!("Boot Image: ");
    let mut image: Option<elf::ElfFile> = None;
    for file in boot_image.files() {
        kprintln!("  {}", file.name());
        let exec_file = elf::ElfFile::new(boot_image.file_data(file));
        image.get_or_insert(exec_file);
    }

    let kernel_parameters = KernelParameters {
        memory_map,
        boot_image: &boot_image,
    };

    // let mut copy_bottom = 0u64;
    // unsafe {
    //     asm!("mov {}, rsp", out(reg) copy_bottom);
    // }

    let process = KernelProcess::from_elf(
        &image.expect("Unable to find kernel image!"),
        0,
        &mut mapper,
        &mut frame_allocator,
    );

    let ptr: *const PageTable = process.address_space.as_ref();

    let frame = match mapper.translate_addr(VirtAddr::new(ptr as u64)) {
        Some(addr) => match PhysFrame::<Size4KiB>::from_start_address(addr) {
            Err(_) => panic!("Unable to get frame! (1)"),
            Ok(frame) => frame,
        },
        None => panic!("Unable to get frame! (2)"),
    };
    
    kprintln!("{:p} {:p}", &memory_map, &frame);

    unsafe {
        asm!("", in("r13") process.stack_base, in("r14") process.entry, in("r15") &kernel_parameters);
        Cr3::write(frame, Cr3Flags::empty());
        asm!("mov rdi, r15");
        asm!("mov rsp, r13");
        asm!("mov rbp, r13");

        // TODO: find a better way to do this
        asm!("jmp r14");
    }

    loop {}
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    kprintln!("LOADER PANIC! {}\n", _info);
    loop {}
}
