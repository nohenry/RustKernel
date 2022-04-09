// use linked_list_allocator::LockedHeap;

use crate::linked_list_allocator::{Heap, LockedHeap};

use x86_64::{
    structures::paging::{
        mapper::MapToError, page::PageRangeInclusive, FrameAllocator, Mapper, Page, PageTableFlags,
        Size4KiB,
    },
    VirtAddr,
};

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100000 * 1024;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

pub fn init_heap(heap: &Heap) {
    unsafe {
        ALLOCATOR.lock().update(heap);
    }
}

pub fn heap_range(offset: usize) -> PageRangeInclusive {
    let heap_start = VirtAddr::new((HEAP_START + offset) as u64);
    let heap_end = heap_start + (HEAP_SIZE + offset) - 1u64;
    let heap_start_page = Page::<Size4KiB>::containing_address(heap_start);
    let heap_end_page = Page::containing_address(heap_end);
    Page::range_inclusive(heap_start_page, heap_end_page)
}

pub fn init_heap_new(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    offset: usize,
    user: bool,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = heap_range(offset);

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | if user {
                PageTableFlags::USER_ACCESSIBLE
            } else {
                PageTableFlags::empty()
            };
        unsafe { mapper.map_to(page, frame, flags, frame_allocator) };
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START + offset, HEAP_SIZE);
    }

    Ok(())
}

pub fn heap_top() -> usize {
    unsafe { ALLOCATOR.lock().top() }
}

pub fn heap() -> Heap {
    unsafe { ALLOCATOR.heap() }
}
