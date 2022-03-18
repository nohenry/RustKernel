use core::{marker::PhantomData};

use crate::mem;
use alloc::{boxed::Box};
// use x86_64::structures::paging::PageTable;
use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB, Translate,
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

#[inline(always)]
pub fn set_syscall_sp() {
    unsafe { asm!("mov {}, rsp", out(reg) SYSCALL_SP) }
    unsafe { kprintln!("SP {:x}", SYSCALL_SP);} 
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

        unsafe {
            for frame in kernel_code_frames {
                mapper
                    .identity_map(
                        frame,
                        PageTableFlags::USER_ACCESSIBLE | PageTableFlags::PRESENT,
                        frame_allocator,
                    )
                    .expect("Unable to identity map!")
                    .flush();
            }
        }

        Process {
            id: 0,
            state: Default::default(),
            entry,
            stack_base: (stack_page.start_address().as_u64() + 4095) as *mut u64,
            address_space: new_page_table,
        }
    }
}

// #[inline(always)]
// pub fn syscall() {
//     unsafe { asm!("syscall") }
// }

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
    loop {
        // Syscall::<SNone>::syscall(SyscallType::Unknown);
        // let string = String::from("Potato\n");
        // let string = "Potato\n";
        // Syscall::<Two>::syscall(
        //     SyscallType::Write,
        //     string.as_ptr() as u64,
        //     string.len() as u64,
        // );
    }
}

unsafe fn syscall_entry_stub() {
    asm!(
        "
        push rax
        push rbx
        push rcx
        push rdx

        push rsi
        push rdi

        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15

        push rbp
        ",
        options(nostack)
    );

    asm!("mov rdx, rsp", options(nostack));
    asm!("mov rsp, {}", in(reg) SYSCALL_SP, options(nostack));

    asm!("mov rsi, cr3", options(nostack));
    asm!("mov cr3, {}", in(reg) mem::KERNEL_MAP, options(nostack));
    asm!("mov {}, rdx", out(reg) SYSCALL_USP, options(nostack));

    asm!("
        push rsi
        call {}
        pop rsi
        ", sym syscall_entry, options(nostack));

    asm!("mov cr3, rsi", options(nostack));
    asm!("mov rsp, {}", in(reg) SYSCALL_USP, options(nostack));

    asm!(
        "
        pop rbp
        
        pop r15
        pop r14
        pop r13
        pop r12
        pop rax
        pop r10
        pop r9
        pop r8

        
        pop rdi
        pop rsi

        pop rdx
        pop rax
        pop rbx
        pop rax
        sysretq
        ",
        options(nostack)
    );
}

fn syscall_entry() {
    let return_address = unsafe {
        let mut val: u32;
        asm!("", out("ecx") val);
        val
    };
    let flags = unsafe {
        let mut val: u64;
        asm!("", out("r11") val);
        val
    };
    let syscall_type: u64;
    let p1: u64;
    let p2: u64;
    let p3: u64;
    let p4: u64;
    unsafe {
        asm!("", out("rdi") syscall_type, out("r8") p1, out("r9") p2, out("r10") p3, out("r12") p4)
    }
    let syscall_type = SyscallType::from(syscall_type);

    match syscall_type {
        SyscallType::Write => {
            let string = unsafe {
                core::str::from_utf8(core::slice::from_raw_parts(p1 as *const u8, p2 as usize))
            }
            .unwrap();
            kprint!("{}", string);
        }
        SyscallType::Unknown => (),
    }

    unsafe { asm!("",  in("ecx") return_address, in("r11") flags ) }
}

#[inline(always)]
pub unsafe fn jump_usermode(mapper: &OffsetPageTable, process: &Process) {
    let sepointer = syscall_entry_stub as u64;
    let lower = (sepointer & 0xFFFFFFFF) as u32;
    let upper = ((sepointer & 0xFFFFFFFF00000000) >> 32) as u32;

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
    ",
    in("eax") lower,
    in("edx") upper,
    in("r11") process.entry,
    in("r12") process.stack_base
    );

    let ptr: *const PageTable = process.address_space.as_ref();

    match mapper.translate_addr(VirtAddr::new(ptr as u64)) {
        Some(addr) => match PhysFrame::<Size4KiB>::from_start_address(addr) {
            Err(_) => {}
            Ok(frame) => Cr3::write(frame, Cr3Flags::empty()),
        },
        None => (),
    }

    asm!(
        "
    mov rcx, r11
    mov rsp, r12
    mov rbp, r12
	mov r11, 0x202 
	sysretq ",
    );
}
