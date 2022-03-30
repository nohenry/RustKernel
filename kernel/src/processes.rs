use core::{arch::asm, marker::PhantomData, sync::atomic::AtomicU32};

use crate::{
    interrupt_begin, interrupt_end,
    interrupts::{CpuSnapshot, IDT},
    mem, elf::{ElfFile, self, SegmentType},
};
use alloc::{boxed::Box, string::String};
// use x86_64::structures::paging::PageTable;
use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::{
        idt::InterruptDescriptorTable,
        paging::{
            FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
            Size4KiB, Translate, mapper::TranslateResult,
        },
    },
    PhysAddr, VirtAddr,
};

use crate::{efi, util};

type ProcessId = u32;

// TODO: why is it 0xffff_8844_4444_0000???
// const PROCESS_CODE_ADDRESS: usize = 0x_8844_4444_0000;
const PROCESS_STACK_ADDRESS: usize = 0x844_4444_0000;

pub static mut SYSCALL_SP: u64 = 0x0;
pub static mut SYSCALL_USP: u64 = 0x0;
pub static mut SYSCALL_UMAP: u64 = 0x0;

static IDINDEX: AtomicU32 = AtomicU32::new(0);

#[inline(always)]
pub fn set_syscall_sp() {
    unsafe { asm!("mov {}, rsp", out(reg) SYSCALL_SP) }
    unsafe {
        kprintln!("SP {:x}", SYSCALL_SP);
    }
}

#[derive(Debug)]
pub struct Process {
    id: ProcessId,
    state: util::CpuState,
    address_space: Box<PageTable>,
    stack_base: *mut u64,
    pub entry: fn(),
}

impl Process {
    pub fn new(entry: fn(), frame_allocator: &mut impl FrameAllocator<Size4KiB>) -> Self {
        let mut new_page_table = Box::new(PageTable::new());

        let mut mapper = unsafe { OffsetPageTable::new(&mut new_page_table, VirtAddr::new(0)) };

        let stack_page =
            Page::<Size4KiB>::containing_address(VirtAddr::new(PROCESS_STACK_ADDRESS as u64));
        let stack_frame = frame_allocator
            .allocate_frame()
            .expect("Unable to allocate page for process stack!");
        let stack_flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        unsafe {
            mapper
                .map_to(stack_page, stack_frame, stack_flags, frame_allocator)
                .expect("Unable to map page for process stack!")
                .flush()
        };

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

        let kernel_stack_start = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(
            unsafe { SYSCALL_SP } - 4 * 4096,
        ));
        let kernel_stack_end =
            PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(unsafe { SYSCALL_SP }));
        let kernel_stack_frames =
            PhysFrame::<Size4KiB>::range_inclusive(kernel_stack_start, kernel_stack_end);

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

            for frame in kernel_stack_frames {
                mapper
                    .identity_map(
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        frame_allocator,
                    )
                    .expect("Unable to identity map!")
                    .flush();
            }
        }

        let id = IDINDEX.load(core::sync::atomic::Ordering::SeqCst);
        IDINDEX.store(id, core::sync::atomic::Ordering::SeqCst);

        Process {
            id,
            state: Default::default(),
            entry,
            stack_base: (stack_page.start_address().as_u64() + 4095) as *mut u64,
            address_space: new_page_table,
        }
    }

    pub fn from_elf(elf: &elf::ElfFile<'_>, current_mapper: &mut impl Mapper<Size4KiB>, frame_allocator: &mut impl FrameAllocator<Size4KiB>) -> Process {
        let mut new_page_table = Box::new(PageTable::new());
        let mut mapper = unsafe { OffsetPageTable::new(&mut new_page_table, VirtAddr::new(0)) };

        // Setup stack
        let stack_page =
            Page::<Size4KiB>::containing_address(VirtAddr::new(PROCESS_STACK_ADDRESS as u64));
        let stack_frame = frame_allocator
            .allocate_frame()
            .expect("Unable to allocate page for process stack!");
        let stack_flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        unsafe {
            mapper
                .map_to(stack_page, stack_frame, stack_flags, frame_allocator)
                .expect("Unable to map page for process stack!")
                .flush()
        };

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

        let kernel_stack_start = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(
            unsafe { SYSCALL_SP } - 4 * 4096,
        ));
        let kernel_stack_end =
            PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(unsafe { SYSCALL_SP }));
        let kernel_stack_frames =
            PhysFrame::<Size4KiB>::range_inclusive(kernel_stack_start, kernel_stack_end);

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

            for frame in kernel_stack_frames {
                mapper
                    .identity_map(
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        frame_allocator,
                    )
                    .expect("Unable to identity map!")
                    .flush();
            }

            /* APIC register mapping for kernel */
            mapper.identity_map(
                PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(0xFEE00000)),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                frame_allocator,
            )
            .expect("Unable to map apic regs for process!")
            .flush();
        }
        // Get new process ID
        let id = IDINDEX.load(core::sync::atomic::Ordering::SeqCst);
        IDINDEX.store(id, core::sync::atomic::Ordering::SeqCst);

        let header = elf.header();
        for pheader in elf.progam_headers() {
            match pheader.segement_type {
                SegmentType::Load => {
                    // Pages of segment virtual address
                    let pg_start = Page::<Size4KiB>::containing_address(VirtAddr::new(
                        pheader.virtual_address,
                    ));
                    let pg_end = Page::<Size4KiB>::containing_address(VirtAddr::new(
                        pheader.virtual_address + pheader.segment_mem_size
                    ));

                    let pages = Page::<Size4KiB>::range_inclusive(pg_start, pg_end);

                    /* Segment Data */
                    let data = elf.segment(pheader);

                    let mut file_size = pheader.segment_file_size;

                    if let Some(data) = data {
                        /* Map virtual pages, allocate physical frames and copy the segment data
                         * from elf file to those frames */
                        for page in pages {
                            let frame = frame_allocator.allocate_frame().expect("Unable to allocate physical frame for elf process!");

                            let frame_addr = unsafe {
                                // Identity map frame in current address space for copying
                                current_mapper.identity_map(frame, PageTableFlags::WRITABLE | PageTableFlags::PRESENT, frame_allocator);
                                
                                core::slice::from_raw_parts_mut(
                                    ((frame.start_address().as_u64() + (pheader.virtual_address - (pheader.virtual_address & !0xFFF)) as u64) as *mut u8),
                                    file_size.min(4096) as usize,
                                )
                            };

                            /* Copy segment data */
                            let offset = (pheader.segment_file_size-file_size) as usize;
                            frame_addr.copy_from_slice(&data[offset..offset + file_size.min(4096) as usize]);

                            /* Map frames in processes new address space*/
                            unsafe { mapper.map_to(page, frame, PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::PRESENT, frame_allocator) }.expect("Unable to map elf segment!").flush();

                            if file_size > 4096 {
                                file_size -= 4096;
                            }
                        }
                    }
                     
                }
                _ => ()
            }
        }

        Process {
            id,
            state: util::CpuState::default(),
            address_space: new_page_table,
            stack_base: (stack_page.start_address().as_u64() + 4095) as *mut u64,
            entry: unsafe { core::mem::transmute(header.entry as *const ()) }
        }
    }
}

enum SyscallType {
    Write,
    Unknown,
}

impl From<u64> for SyscallType {
    fn from(a: u64) -> Self {
        match a {
            0 => SyscallType::Write,
            _ => SyscallType::Unknown,
        }
    }
}

struct Syscall<'a, T> {
    phantom: PhantomData<&'a T>,
}

struct SNone;
struct One;
struct Two;
struct Three;
struct Four;

impl<'a> Syscall<'a, SNone> {
    #[inline]
    fn syscall(stype: SyscallType) {
        unsafe { asm!("syscall", in("rax") stype as u64) }
    }
}

impl<'a> Syscall<'a, One> {
    #[inline]
    fn syscall<A>(stype: SyscallType, a: A)
    where
        A: Into<u64>,
    {
        unsafe { asm!("syscall", in("rdi") stype as u64, in("r8") a.into()) }
    }
}
impl<'a> Syscall<'a, Two> {
    #[inline]
    fn syscall<A, B>(stype: SyscallType, a: A, b: B)
    where
        A: Into<u64>,
        B: Into<u64>,
    {
        unsafe { asm!("syscall", in("rdi") stype as u64,in("r8") a.into(), in("r9") b.into()) }
    }
}
impl<'a> Syscall<'a, Three> {
    #[inline]
    fn syscall<A, B, C>(stype: SyscallType, a: A, b: B, c: C)
    where
        A: Into<u64>,
        B: Into<u64>,
        C: Into<u64>,
    {
        unsafe {
            asm!("syscall", in("rdi") stype as u64,in("r8") a.into(), in("r9") b.into(), in("r10") c.into())
        }
    }
}
impl<'a> Syscall<'a, Four> {
    #[inline]
    fn syscall<A, B, C, D>(stype: SyscallType, a: A, b: B, c: C, d: D)
    where
        A: Into<u64>,
        B: Into<u64>,
        C: Into<u64>,
        D: Into<u64>,
    {
        unsafe {
            asm!("syscall", in("rdi") stype as u64,in("r8") a.into(), in("r9") b.into(), in("r10") c.into(), in("r12") d.into())
        }
    }
}

pub fn test_process() {
    // let pf = idt.page_fault;
    loop {
        Syscall::<SNone>::syscall(SyscallType::Unknown);
        // let string = "Potato\n";
        // Syscall::<Two>::syscall(
        //     SyscallType::Write,
        //     string.as_ptr() as u64,
        //     string.len() as u64,
        // );
    }
}

// #[naked]
unsafe extern "C" fn syscall_entry_stub() {
    interrupt_begin!();

    let mut cpu: *const CpuSnapshot = core::ptr::null_mut();

    asm!("mov rdx, rsp", options(nomem, nostack)); // Save old stack
    asm!("mov rsp, rax; mov rbp, rsp", in("rax") SYSCALL_SP, options(nostack)); // Load kernel stack
    asm!("", out("rdx") SYSCALL_USP, options(nostack)); // Save old stack into variable for later
    asm!("", out("rdx") cpu, options(nostack)); // Load address into pointer for cpu snapshot

    asm!("mov rax, cr3", out("rax") SYSCALL_UMAP, options(nostack)); // Save old address space

    syscall_entry(&*cpu);

    asm!("mov cr3, rax", in("rax") SYSCALL_UMAP, options(nostack));
    asm!("mov rsp, rax", in("rax") SYSCALL_USP, options(nostack));

    interrupt_end!();

    asm!("sysretq", options(noreturn));
}

fn syscall_entry(cpu: &CpuSnapshot) {
    // kprint!(".")
    // let syscall_type = cpu.rdi;
    // let syscall_type = SyscallType::from(syscall_type);

    // match syscall_type {
    //     SyscallType::Write => {
    //         let string = unsafe {
    //             core::str::from_utf8(core::slice::from_raw_parts(cpu.r8 as *const u8, cpu.r9 as usize))
    //         }
    //         .unwrap();
    //         kprint!("{}", string);
    //     }
    //     SyscallType::Unknown => (),
    // }
}

#[inline(never)]
pub unsafe fn jump_usermode(mapper: &OffsetPageTable, process: &Process) {
    let sepointer = syscall_entry_stub as u64;
    let lower = (sepointer & 0xFFFFFFFF) as u32;
    let upper = ((sepointer & 0xFFFFFFFF00000000) >> 32) as u32;

    let ptr: *const PageTable = process.address_space.as_ref();

    let frame = match mapper.translate_addr(VirtAddr::new(ptr as u64)) {
        Some(addr) => match PhysFrame::<Size4KiB>::from_start_address(addr) {
            Err(_) => panic!("Unable to get frame! (1)"),
            Ok(frame) => frame,
        },
        None => panic!("Unable to get frame! (2)"),
    };

    asm!(
        "
    mov rcx, 0xc0000082 
    wrmsr               
    mov rcx, 0xc0000080 
    rdmsr               
    or eax, 1           
    wrmsr               
    mov rcx, 0xc0000081 
    rdmsr               
    mov edx, 0x00180008
    wrmsr           	
    mov eax, 0x200
    mov rcx, 0xc0000084
    wrmsr
    ",
    in("eax") lower,
    in("edx") upper,
    in("r11") process.entry,
    in("r12") process.stack_base
    );

    Cr3::write(frame, Cr3Flags::empty());

    asm!(
        "
    mov rcx, r11
    mov rsp, r12
    mov rbp, r12
	mov r11, 0x202 
	sysretq ",
        options(noreturn)
    );
}
