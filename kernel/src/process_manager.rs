use core::arch::asm;

use crate::interrupts;
use alloc::{collections::LinkedList, vec::Vec};
use bitflags::bitflags;
use common::{
    elf, kprintln,
    process::Process,
    x86_64::{
        registers::control::{Cr3, Cr3Flags},
        structures::paging::{Mapper, OffsetPageTable, PageTable, PhysFrame, Size4KiB, Translate},
        VirtAddr,
    },
};

pub enum State {
    Ready,
    Blocked,
    Running,
}

static mut PROCESSES: Vec<ManagedProcess> = Vec::new();
static mut NEXT_PROCESS: usize = 0;

bitflags! {
    struct ProcessFlags: u32 {
        const KERNEL = 1;
    }
}

pub struct ManagedProcess {
    process: Process,
    state: State,
    flags: ProcessFlags,
}

impl ManagedProcess {
    pub fn new_kernel_process(
        elf: &elf::ElfFile<'_>,
        kernel: &elf::ElfFile<'_>,
        kernel_stack_start: u64,
        kernel_stack_end: u64,
        mem_size: usize,
    ) -> ManagedProcess {
        let mut current_mapper =
            common::mem::active_offset_page_table(common::memory_regions::PAGE_TABLE_OFFSET);
        ManagedProcess {
            process: Process::from_elf(
                elf,
                kernel,
                kernel_stack_start,
                kernel_stack_end,
                mem_size,
                &mut current_mapper,
                common::mem::allocator().get_mut(),
            ),
            state: State::Ready,
            flags: ProcessFlags::KERNEL,
        }
    }

    pub fn spawn(self) {
        unsafe {
            PROCESSES.push(self);
        }
    }

    pub fn load(&self) {
        let ptr: *const PageTable = self.process.address_space.as_ref();

        let frame = match <OffsetPageTable as Translate>::translate_addr(
            &mut crate::mem::active_offset_page_table(common::memory_regions::PAGE_TABLE_OFFSET),
            VirtAddr::new(ptr as u64),
        ) {
            Some(addr) => match PhysFrame::<Size4KiB>::from_start_address(addr) {
                Err(_) => panic!("Unable to get frame! (1)"),
                Ok(frame) => frame,
            },
            None => panic!("Unable to get frame! (2)"),
        };

        unsafe {
            Cr3::write(frame, Cr3Flags::empty());
        }
    }
}

pub fn init() {
    interrupts::register_handler(0x3C, schedular);
}

pub fn schedular(frame: &mut interrupts::InterruptStackFrame, snapshot: &interrupts::CpuSnapshot) {
    kprintln!("Scheduling");
    unsafe {
        if PROCESSES.len() > 0 {
            let next = &PROCESSES[NEXT_PROCESS];
            NEXT_PROCESS += 1;
        }
    }
}
