use alloc::boxed::Box;
use x86_64::{
    structures::paging::{
        page::PageRangeInclusive, FrameAllocator, Mapper, OffsetPageTable, Page, PageTable,
        PageTableFlags, PhysFrame, Size1GiB, Size4KiB, Size2MiB,
    },
    PhysAddr, VirtAddr,
};

use crate::{
    efi,
    elf::{self, SegmentType},
    mem,
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
            Page::containing_address(VirtAddr::new(PROCESS_STACK_ADDRESS as u64 - 10 * 4096));
        Page::range_inclusive(stack_page_end, stack_page_start)
    }

    pub fn from_elf(
        elf: &elf::ElfFile<'_>,
        kernel_stack_start: u64,
        kernel_stack_end: u64,
        mem: usize,
        current_mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> KernelProcess {
        let mut new_page_table = Box::new(PageTable::new());
        let mut mapper = unsafe { OffsetPageTable::new(&mut new_page_table, VirtAddr::new(0)) };

        let phys_mem_start = PhysFrame::containing_address(PhysAddr::zero());
        let phys_mem_end = PhysFrame::containing_address(PhysAddr::new(mem as _));
        let phys_frames = PhysFrame::<Size1GiB>::range_inclusive(phys_mem_start, phys_mem_end);

        // let phys_mem_pg_start = Page::containing_address(VirtAddr::new(mem::PAGE_TABLE_OFFSET));
        // let phys_mem_pg_end =
        //     Page::containing_address(VirtAddr::new(mem::PAGE_TABLE_OFFSET + mem as u64));
        // let phys_pg_frames = Page::<Size1GiB>::range_inclusive(phys_mem_pg_start, phys_mem_pg_end);

        unsafe {
            for frame in phys_frames {
                kprintln!("Frame {:x?}", frame);
                let page = Page::containing_address(VirtAddr::new(mem::PAGE_TABLE_OFFSET) + frame.start_address().as_u64());
                mapper
                    .map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        frame_allocator,
                    )
                    .expect("Unable to identity map!").ignore();
            }
        }

        // Setup stack
        let stack_pages = KernelProcess::get_stack();
        let stack_flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

        for page in stack_pages {
            kprintln!("Process Stack {:?}", page);
            let stack_frame = frame_allocator
                .allocate_frame()
                .expect("Unable to allocate page for process stack!");
            unsafe {
                mapper
                    .map_to(page, stack_frame, stack_flags, frame_allocator)
                    .expect("Unable to map page for process stack!")
                    // .flush()
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

        let kernel_stack_end =
            PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(kernel_stack_end));
        let kernel_stack_start =
            PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(kernel_stack_start));
        let kernel_stack_frames =
            PhysFrame::<Size4KiB>::range_inclusive(kernel_stack_end, kernel_stack_start);

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
                    .expect("Unable to identity map!");
            }

            for frame in kernel_stack_frames {
                mapper
                    .identity_map(
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::WRITABLE,
                        frame_allocator,
                    )
                    .expect("Unable to identity map!");
            }

            /* APIC register mapping for kernel */
            mapper
                .identity_map(
                    PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(0xFEE00000)),
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .expect("Unable to map apic regs for process!");
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
            stack_base: PROCESS_STACK_ADDRESS as *mut u64,
            entry: unsafe { core::mem::transmute(header.entry as *const ()) },
        }
    }

    pub fn get_pt(&mut self) -> OffsetPageTable {
        unsafe { OffsetPageTable::new(self.address_space.as_mut(), VirtAddr::new(0)) }
    }
}
