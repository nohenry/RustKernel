use alloc::boxed::Box;
use x86_64::{
    structures::paging::{
        page::PageRangeInclusive, FrameAllocator, Mapper, OffsetPageTable, Page, PageTable,
        PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::{
    efi,
    elf::{self, SegmentType},
};

const PROCESS_STACK_ADDRESS: usize = 0x844_4444_0000;

#[derive(Debug)]
pub struct KernelProcess {
    pub address_space: Box<PageTable>,
    pub stack_base: *mut u64,
    pub entry: fn(),
}

impl KernelProcess {
    pub fn get_stack() -> PageRangeInclusive {
        let stack_page_start =
            Page::containing_address(VirtAddr::new(PROCESS_STACK_ADDRESS as u64));

        let stack_page_end =
            Page::containing_address(VirtAddr::new(PROCESS_STACK_ADDRESS as u64 - 4 * 4096));
        let stack_pages = Page::range_inclusive(stack_page_end, stack_page_end);
    }

    pub fn from_elf(
        elf: &elf::ElfFile<'_>,
        kernel_stack: u64,
        current_mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> KernelProcess {
        let mut new_page_table = Box::new(PageTable::new());
        let mut mapper = unsafe { OffsetPageTable::new(&mut new_page_table, VirtAddr::new(0)) };

        // Setup stack
        let stack_pages = KernelProcess::get_stack();
        let stack_flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

        for page in stack_pages {
            let stack_frame = frame_allocator
                .allocate_frame()
                .expect("Unable to allocate page for process stack!");
            unsafe {
                mapper
                    .map_to(page, stack_frame, stack_flags, frame_allocator)
                    .expect("Unable to map page for process stack!")
                    .flush()
            };
        }

        /* Map kernel crap for syscalls and interrupts */
        let kernel_code_descriptor = unsafe {
            efi::DESCRIPTORS
                .iter()
                .find(|d| matches!(d.memory_type, efi::MemoryType::LoaderCode))
                .expect("Unable to find loader code!")
        };
        let kernel_code_start = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(
            kernel_code_descriptor.physical_address as u64,
        ));
        let kernel_code_end = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(
            kernel_code_descriptor.physical_address as u64
                + kernel_code_descriptor.size as u64 * 4096,
        ));
        let kernel_code_frames =
            PhysFrame::<Size4KiB>::range_inclusive(kernel_code_start, kernel_code_end);

        // let kernel_stack_start =
        //     PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(kernel_stack - 4 * 4096));
        // let kernel_stack_end =
        //     PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(kernel_stack));
        // let kernel_stack_frames =
        //     PhysFrame::<Size4KiB>::range_inclusive(kernel_stack_start, kernel_stack_end);

        unsafe {
            for frame in kernel_code_frames {
                mapper
                    .identity_map(
                        frame,
                        // For now the test process is in kernel code so user accessable flag is set
                        PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE,
                        frame_allocator,
                    )
                    .expect("Unable to identity map!")
                    .flush();
            }

            // for frame in kernel_stack_frames {
            //     mapper
            //         .identity_map(
            //             frame,
            //             PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            //             frame_allocator,
            //         )
            //         .expect("Unable to identity map!")
            //         .flush();
            // }

            /* APIC register mapping for kernel */
            mapper
                .identity_map(
                    PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(0xFEE00000)),
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .expect("Unable to map apic regs for process!")
                .flush();
        }

        let header = elf.header();
        for pheader in elf.progam_headers() {
            match pheader.segement_type {
                SegmentType::Load => {
                    // Pages of segment virtual address
                    let pg_start = Page::<Size4KiB>::containing_address(VirtAddr::new(
                        pheader.virtual_address,
                    ));
                    let pg_end = Page::<Size4KiB>::containing_address(VirtAddr::new(
                        pheader.virtual_address + pheader.segment_mem_size,
                    ));

                    let pages = Page::<Size4KiB>::range_inclusive(pg_start, pg_end);

                    /* Segment Data */
                    let data = elf.segment(pheader);

                    let mut file_size = pheader.segment_file_size;

                    if let Some(data) = data {
                        /* Map virtual pages, allocate physical frames and copy the segment data
                         * from elf file to those frames */
                        for page in pages {
                            let frame = frame_allocator
                                .allocate_frame()
                                .expect("Unable to allocate physical frame for elf process!");

                            let frame_addr = unsafe {
                                // Identity map frame in current address space for copying
                                current_mapper.identity_map(
                                    frame,
                                    PageTableFlags::WRITABLE | PageTableFlags::PRESENT,
                                    frame_allocator,
                                );

                                core::slice::from_raw_parts_mut(
                                    ((frame.start_address().as_u64()
                                        + (pheader.virtual_address
                                            - (pheader.virtual_address & !0xFFF))
                                            as u64)
                                        as *mut u8),
                                    file_size.min(4096) as usize,
                                )
                            };

                            /* Copy segment data */
                            let offset = (pheader.segment_file_size - file_size) as usize;
                            frame_addr.copy_from_slice(
                                &data[offset..offset + file_size.min(4096) as usize],
                            );

                            /* Map frames in processes new address space*/
                            unsafe {
                                mapper.map_to(
                                    page,
                                    frame,
                                    PageTableFlags::WRITABLE
                                        | PageTableFlags::USER_ACCESSIBLE
                                        | PageTableFlags::PRESENT,
                                    frame_allocator,
                                )
                            }
                            .expect("Unable to map elf segment!")
                            .flush();

                            if file_size > 4096 {
                                file_size -= 4096;
                            }
                        }
                    }
                }
                _ => (),
            }
        }

        KernelProcess {
            address_space: new_page_table,
            stack_base: stack_page.start_address().as_u64() as *mut u64,
            entry: unsafe { core::mem::transmute(header.entry as *const ()) },
        }
    }
}
