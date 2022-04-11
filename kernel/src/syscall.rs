use core::{arch::asm, marker::PhantomData};
use common::{process::{self, Process}, x86_64::{structures::paging::{OffsetPageTable, PageTable, PhysFrame, Size4KiB, Translate}, VirtAddr, registers::control::{Cr3, Cr3Flags}}};

use crate::{interrupt_begin, interrupt_end, interrupts::CpuSnapshot};

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

// #[naked]
unsafe extern "C" fn syscall_entry_stub() {
    interrupt_begin!();

    let mut cpu: *const CpuSnapshot = core::ptr::null_mut();

    asm!("mov rdx, rsp", options(nomem, nostack)); // Save old stack
    asm!("mov rsp, rax; mov rbp, rsp", in("rax") process::SYSCALL_SP, options(nostack)); // Load kernel stack
    asm!("", out("rdx") process::SYSCALL_USP, options(nostack)); // Save old stack into variable for later
    asm!("", out("rdx") cpu, options(nostack)); // Load address into pointer for cpu snapshot

    asm!("mov rax, cr3", out("rax") process::SYSCALL_UMAP, options(nostack)); // Save old address space

    syscall_entry(&*cpu);

    asm!("mov cr3, rax", in("rax") process::SYSCALL_UMAP, options(nostack));
    asm!("mov rsp, rax", in("rax") process::SYSCALL_USP, options(nostack));

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
