use x86_64::{
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

use crate::efi;

pub const STACK_SIZE: usize = 4096 * 5;

pub static mut KERNEL_MAP: u64 = 0x0;

pub unsafe fn active_level_4_table() -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = VirtAddr::new(0) + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

pub unsafe fn init() -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table();
    OffsetPageTable::new(level_4_table, VirtAddr::new(0))
}

pub struct PageTableFrameAllocator {
    memory_map: efi::MemoryMap,
    next: usize,
}

impl PageTableFrameAllocator {
    pub fn new(memory_map: efi::MemoryMap) -> Self {
        PageTableFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let iter = self.memory_map.iter();
        let usable = iter.filter(|d| d.memory_type.is_usable());

        let address_range =
            usable.map(|u| u.physical_address..(u.physical_address + u.size * 4096));
        let addresses = address_range.flat_map(|r| r.step_by(4096));
        addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr as u64)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for PageTableFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
