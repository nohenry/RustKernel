use core::{
    iter::{Filter, FlatMap, Map, StepBy},
    mem::{size_of, size_of_val},
    ops::Range,
    slice::Iter,
};

use spinning_top::{lock_api::MutexGuard, RawSpinlock, Spinlock};
use x86_64::{
    structures::paging::{
        mapper::{MapToError, MapperFlush, MapperFlushAll},
        FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
        PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::{efi::{self, MemoryDescriptor}, memory_regions::PAGE_TABLE_OFFSET};

pub const STACK_SIZE: usize = 4096 * 5;

pub static mut KERNEL_MAP: u64 = 0x0;

static mut ALLOCATOR: Option<Spinlock<PageTableFrameAllocator<'static>>> = None;

pub fn allocator() -> &'static mut Spinlock<PageTableFrameAllocator<'static>> {
    unsafe { ALLOCATOR.as_mut().unwrap() }
}

pub unsafe fn active_level_4_table() -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = VirtAddr::new(0) + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

pub fn active_offset_page_table(offset: u64) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table();
        OffsetPageTable::new(level_4_table, VirtAddr::new(offset))
    }
}

pub fn init(alloc: PageTableFrameAllocator<'static>, offset: u64) -> OffsetPageTable<'static> {
    unsafe {
        ALLOCATOR.replace(Spinlock::new(alloc));
    }

    active_offset_page_table(offset)
}

pub fn map_virt<'a, S>(
    phys: PhysAddr,
    virt: VirtAddr,
    size: usize,
) -> Result<(), MapToError<S>>
where
    S: PageSize, OffsetPageTable<'a>: Mapper<S>
{
    let mut pt = active_offset_page_table(PAGE_TABLE_OFFSET);
    let start = PhysFrame::containing_address(phys);
    let end = PhysFrame::containing_address(phys + size);

    let pg_start = Page::containing_address(virt);
    let pg_end = Page::containing_address(virt + size);

    for (frame, page) in PhysFrame::<S>::range_inclusive(start, end)
        .zip(Page::<S>::range_inclusive(pg_start, pg_end))
    {
        match unsafe {
            <OffsetPageTable as Mapper<S>>::map_to(
                &mut pt,
                page,
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                allocator().get_mut(),
            )
        } {
            Ok(o) => o.flush(),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn map_phys(phys: PhysAddr, size: usize) -> Result<(), MapToError<Size4KiB>> {
    let mut pt = active_offset_page_table(PAGE_TABLE_OFFSET);
    let start = PhysFrame::containing_address(phys);
    let end = PhysFrame::containing_address(phys + size);
    for frame in PhysFrame::<Size4KiB>::range_inclusive(start, end) {
        match unsafe {
            <OffsetPageTable as Mapper<Size4KiB>>::identity_map(
                &mut pt,
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                allocator().get_mut(),
            )
        } {
            Ok(o) => o.flush(),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn map_phys_table(
    pgtbl: &mut OffsetPageTable<'_>,
    phys: PhysAddr,
    size: usize,
) -> Result<(), MapToError<Size4KiB>> {
    kprintln!("Mapping {:x}", phys.as_u64());
    let start = PhysFrame::containing_address(phys);
    let end = PhysFrame::containing_address(phys + size);
    for frame in PhysFrame::<Size4KiB>::range_inclusive(start, end) {
        match unsafe {
            pgtbl.identity_map(
                PhysFrame::<Size4KiB>::containing_address(phys),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                allocator().get_mut(),
            )
        } {
            Ok(o) => o.flush(),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

// pub fn map_ptr<T: ?Sized>(ptr: *const T) -> Result<MapperFlush<Size4KiB>, MapToError<Size4KiB>> {
//     map_phys(PhysAddr::new(ptr as u64), size_of::<T>())
// }

pub fn map_ref<T: ?Sized>(ptr: &T) -> Result<(), MapToError<Size4KiB>> {
    map_phys(
        PhysAddr::new(ptr as *const T as *const () as u64),
        size_of_val(ptr),
    )
}

pub fn map_ref_len<T: ?Sized>(ptr: &T, size: usize) -> Result<(), MapToError<Size4KiB>> {
    map_phys(
        PhysAddr::new(ptr as *const T as *const () as u64),
        size_of_val(ptr) * size,
    )
}

pub fn map_arr<T>(ptr: &[T]) -> Result<(), MapToError<Size4KiB>> {
    map_phys(
        PhysAddr::new(&ptr[0] as *const T as u64),
        size_of::<T>() * ptr.len(),
    )
}

pub fn map_arr_len<T>(ptr: &[T], len: usize) -> Result<(), MapToError<Size4KiB>> {
    map_phys(
        PhysAddr::new(&ptr[0] as *const T as u64),
        size_of::<T>() * len,
    )
}

pub fn map_ptr_table<T>(
    pgtbl: &mut OffsetPageTable<'_>,
    ptr: &T,
) -> Result<(), MapToError<Size4KiB>> {
    map_phys_table(pgtbl, PhysAddr::new(ptr as *const T as u64), size_of::<T>())
}

pub fn map_arr_table<T>(
    pgtbl: &mut OffsetPageTable<'_>,
    ptr: &[T],
) -> Result<(), MapToError<Size4KiB>> {
    map_phys_table(
        pgtbl,
        PhysAddr::new(&ptr[0] as *const T as u64),
        size_of::<T>() * ptr.len(),
    )
}

#[derive(Clone)]
pub struct PageTableFrameAllocator<'a> {
    memory_map: efi::MemoryMap<'a>,
    addresses: Map<
        FlatMap<
            Map<
                Filter<Iter<'a, MemoryDescriptor>, fn(&&MemoryDescriptor) -> bool>,
                fn(&MemoryDescriptor) -> Range<usize>,
            >,
            StepBy<Range<usize>>,
            fn(Range<usize>) -> StepBy<Range<usize>>,
        >,
        fn(usize) -> PhysFrame<Size4KiB>,
    >,
}

impl<'a> PageTableFrameAllocator<'a> {
    pub fn swap_map(&mut self, memory_map: efi::MemoryMap<'a>) {
        let curr_frame = self.allocate_frame();
        kprintln!("Frame {:?}", curr_frame);
        let iter = memory_map.iter();
        let usable: Filter<Iter<MemoryDescriptor>, fn(&&MemoryDescriptor) -> bool> =
            iter.filter(|d| d.memory_type.is_usable());

        let mut address_range: Map<
            Filter<Iter<MemoryDescriptor>, fn(&&MemoryDescriptor) -> bool>,
            fn(&MemoryDescriptor) -> Range<usize>,
        > = usable.map(|u| u.physical_address..(u.physical_address + u.size * 4096));
        // address_range.(|f| {f.start < curr_frame.unwrap().start_address().as_u64() as usize});
        while address_range.next().unwrap().start
            < curr_frame.unwrap().start_address().as_u64() as usize
        {}
        let addresses: FlatMap<
            Map<
                Filter<Iter<MemoryDescriptor>, fn(&&MemoryDescriptor) -> bool>,
                fn(&MemoryDescriptor) -> Range<usize>,
            >,
            StepBy<Range<usize>>,
            fn(Range<usize>) -> StepBy<Range<usize>>,
        > = address_range.flat_map(|r| r.step_by(4096));

        let amap: Map<
            FlatMap<
                Map<
                    Filter<Iter<MemoryDescriptor>, fn(&&MemoryDescriptor) -> bool>,
                    fn(&MemoryDescriptor) -> Range<usize>,
                >,
                StepBy<Range<usize>>,
                fn(Range<usize>) -> StepBy<Range<usize>>,
            >,
            fn(usize) -> PhysFrame<Size4KiB>,
        > = addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr as u64)));

        self.addresses = amap;
    }

    pub fn new(memory_map: efi::MemoryMap<'a>) -> Self {
        let iter = memory_map.iter();
        let usable: Filter<Iter<MemoryDescriptor>, fn(&&MemoryDescriptor) -> bool> =
            iter.filter(|d| d.memory_type.is_usable());

        let address_range: Map<
            Filter<Iter<MemoryDescriptor>, fn(&&MemoryDescriptor) -> bool>,
            fn(&MemoryDescriptor) -> Range<usize>,
        > = usable.map(|u| u.physical_address..(u.physical_address + u.size * 4096));
        let addresses: FlatMap<
            Map<
                Filter<Iter<MemoryDescriptor>, fn(&&MemoryDescriptor) -> bool>,
                fn(&MemoryDescriptor) -> Range<usize>,
            >,
            StepBy<Range<usize>>,
            fn(Range<usize>) -> StepBy<Range<usize>>,
        > = address_range.flat_map(|r| r.step_by(4096));

        let amap: Map<
            FlatMap<
                Map<
                    Filter<Iter<MemoryDescriptor>, fn(&&MemoryDescriptor) -> bool>,
                    fn(&MemoryDescriptor) -> Range<usize>,
                >,
                StepBy<Range<usize>>,
                fn(Range<usize>) -> StepBy<Range<usize>>,
            >,
            fn(usize) -> PhysFrame<Size4KiB>,
        > = addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr as u64)));

        PageTableFrameAllocator {
            memory_map,
            addresses: amap,
        }
    }

    pub fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> + 'a {
        let iter = self.memory_map.iter();
        let usable = iter.filter(|d| d.memory_type.is_usable());

        let address_range =
            usable.map(|u| u.physical_address..(u.physical_address + u.size * 4096));
        let addresses = address_range.flat_map(|r| r.step_by(4096));
        addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr as u64)))
    }

    pub fn allocate_size(&mut self, size: usize) -> Option<(PhysFrame<Size4KiB>, usize)> {
        let n = size / 4096;
        let mut ret_frame = PhysFrame::containing_address(PhysAddr::new(0));
        for i in 0..n {
            if let Some(f) = self.allocate_frame() {
                if i == 0 {
                    ret_frame = f
                }
            } else {
                return None;
            }
        }
        Some((ret_frame, n))
    }
}

unsafe impl<'a> FrameAllocator<Size4KiB> for PageTableFrameAllocator<'a> {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.addresses.next();
        frame
    }
}
