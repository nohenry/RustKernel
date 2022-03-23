use core::borrow::Borrow;
use bit_field::BitField;

use crate::{
    acpi::{
        get_xsdt,
        madt::{self, Entry},
        Signature, RSDP,
    },
    drivers::keyboard::Keyboard,
    gdt,
    processes::{SYSCALL_SP, SYSCALL_UMAP, SYSCALL_USP},
    util::{self, in8, out8},
};
use lazy_static::lazy_static;
use x86_64::{
    registers::model_specific::{Msr, IA32_APIC_BASE},
    structures::idt::{self, InterruptDescriptorTable},
};

use crate::drivers::pic::ChainedPics;
use spin;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static APIC: spin::Mutex<LocalApic> = spin::Mutex::new(LocalApic::new());
pub static IOAPIC: spin::Mutex<IOApic> = spin::Mutex::new(IOApic::new());

#[repr(C)]
pub struct CpuSnapshot {
    pub rbp: u64,

    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,

    pub rdi: u64,
    pub rsi: u64,

    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
}

#[macro_export]
macro_rules! interrupt_begin {
    () => {
        unsafe {
        asm!(
        "
        cli
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
        options(nomem, nostack)
    );

    // if crate::processes::SYSCALL_SP != 0 {
    //     asm!("mov rdx, rsp", options(nostack));
    //     asm!("mov rsp, {}", in(reg) crate::processes::SYSCALL_SP, options(nostack));
    //     asm!("mov rsi, cr3", options(nostack));
    //     asm!("mov cr3, {}", in(reg) crate::mem::KERNEL_MAP, options(nostack));
    //     asm!("mov {}, rdx ; push rsi", out(reg) crate::processes::SYSCALL_USP, options(nostack));
    // }

}
    };
}

#[macro_export]
macro_rules! interrupt_end {
    () => {
        unsafe {
            //     if crate::processes::SYSCALL_USP != 0 {
            //     asm!("pop rsi ; mov cr3, rsi", options(nostack));
            //     asm!("mov rsp, {}", in(reg) crate::processes::SYSCALL_USP, options(nostack));
            // }

            asm!(
                "
        pop rbp
        
        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8

        
        pop rdi
        pop rsi

        pop rdx
        pop rcx
        pop rbx
        pop rax
        ",
                options(nomem, nostack)
            );
        }
    };
    (sti) => {
        unsafe {
            //     if crate::processes::SYSCALL_USP != 0 {
            //     asm!("pop rsi ; mov cr3, rsi", options(nostack));
            //     asm!("mov rsp, {}", in(reg) crate::processes::SYSCALL_USP, options(nostack));
            // }

            asm!(
                "
        pop rbp
        
        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8

        
        pop rdi
        pop rsi

        pop rdx
        pop rcx
        pop rbx
        pop rax
        sti
        ",
                options(nomem, nostack)
            );
        }
    };
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
    Cascade,
    SerialPort1,
    SerialPort2,
    ParallelPort1,
    FloppyDisk,
    ParallelPort2,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.general_protection_fault
            .set_handler_fn(general_protection_handler);
        idt.non_maskable_interrupt.set_handler_fn(nmi_handler);
        idt.segment_not_present
            .set_handler_fn(segment_not_present_handler);

        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            idt.page_fault
                .set_handler_fn(pagefault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }

        unsafe {
            idt[InterruptIndex::Timer.as_usize()]
                .set_handler_fn(timer_handler_stub)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            idt[InterruptIndex::Keyboard.as_usize()]
                .set_handler_fn(keyboard_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[InterruptIndex::SerialPort2.as_usize()].set_handler_fn(serial1_handler);
        idt[InterruptIndex::SerialPort1.as_usize()].set_handler_fn(serial1_handler);

        idt[60].set_handler_fn(lapic_timer);
        idt[0xFF].set_handler_fn(lapic_spurious);
        idt
    };
}

pub fn init() {
    x86_64::instructions::interrupts::disable();

    IDT.load();

    APIC.lock().init();
    IOAPIC.lock().init();

    unsafe {
        out8(0x21, 0xFF);
        out8(0xA1, 0xFF);
    }

    x86_64::instructions::interrupts::enable();
}

extern "x86-interrupt" fn timer_handler_stub(stack_frame: idt::InterruptStackFrame) {
    // x86_64::instructions::hlt();
    interrupt_begin!();
    unsafe {
        asm!("mov rdx, rsp", options(nomem, nostack)); // Save old stack
        asm!("mov rsp, rax; mov rbp, rsp", in("rax") SYSCALL_SP, options(nostack)); // Load kernel stack
        asm!("", out("rdx") SYSCALL_USP, options(nostack)); // Save old stack into variable for later
                                                            // asm!("", out("rdx") cpu, options(nostack)); // Load address into pointer for cpu snapshot

        asm!("mov rax, cr3", out("rax") SYSCALL_UMAP, options(nostack)); // Save old address space

        timer_handler();

        asm!("mov cr3, rax", in("rax") SYSCALL_UMAP, options(nostack));
        asm!("mov rsp, rax", in("rax") SYSCALL_USP, options(nostack));

        // PICS.lock()
        //     .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
        // PICS.force_unlock();
    }
    interrupt_end!(sti);
}

fn timer_handler() {}

extern "x86-interrupt" fn serial1_handler(_stack_frame: idt::InterruptStackFrame) {
    kprintln!("Serial\n");
    unsafe {
        util::in8(0x3F8);
    }
    unsafe {
        // PICS.lock()
        //     .notify_end_of_interrupt(InterruptIndex::SerialPort2.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_handler(_stack_frame: idt::InterruptStackFrame) {
    interrupt_begin!();
    unsafe {
        asm!("mov rdx, rsp", options(nostack));
        asm!("mov rsp, {}", in(reg) crate::processes::SYSCALL_SP, options(nostack));

        asm!("mov rsi, cr3", options(nostack));
        // asm!("mov cr3, {}", in(reg) crate::mem::KERNEL_MAP, options(nostack));
        asm!("mov {}, rdx", out(reg) crate::processes::SYSCALL_USP, options(nostack));

        asm!("push rsi", options(nostack));

        let b = in8(0x60);
        let chr = Keyboard::code_to_char(b);
        kprint!("{}", chr);

        asm!("pop rsi", options(nostack));
        asm!("mov cr3, rsi", options(nostack));
        asm!("mov rsp, {}", in(reg) crate::processes::SYSCALL_USP, options(nostack));

        // PICS.lock()
        //     .notify_end_of_interrupt(InterruptIndex::Timer.as_u9());
    }
    interrupt_end!();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: idt::InterruptStackFrame) {
    kprintln!("EXCPETION: BREAKPOINT\n{:#?}\n", stack_frame);
}

extern "x86-interrupt" fn nmi_handler(stack_frame: idt::InterruptStackFrame) {
    kprintln!("EXCPETION: NMI\n{:#?}\n", stack_frame);
}

extern "x86-interrupt" fn segment_not_present_handler(
    stack_frame: idt::InterruptStackFrame,
    e: u64,
) {
}

extern "x86-interrupt" fn general_protection_handler(
    stack_frame: idt::InterruptStackFrame,
    _e: u64,
) {
    kprintln!("EXCPETION: GP\n{:#?}\n{}\n", stack_frame, _e);
    loop {}
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: idt::InterruptStackFrame,
    _e: u64,
) -> ! {
    kprintln!("EXCPETION: Double Fault\n{:#?}\n{}\n", stack_frame, _e);
    loop {}
}

extern "x86-interrupt" fn pagefault_handler(
    _stack_frame: idt::InterruptStackFrame,
    _error_code: idt::PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    kprintln!(
        "EXCPETION: PAGE FAULT\n{:#?}\n{:#?}\n",
        _stack_frame,
        _error_code
    );
    kprintln!("Address: {:?}\n", Cr2::read());

    loop {}
}

extern "x86-interrupt" fn lapic_timer(_stack_frame: idt::InterruptStackFrame) {
    // kprint!(".");
    APIC.lock().send_eoi();
}

extern "x86-interrupt" fn lapic_spurious(_stack_frame: idt::InterruptStackFrame) {
    kprintln!("LAPIC Spurious");
    APIC.lock().send_eoi();
}

pub struct LocalApic {
    msr: Msr,
    base: u64,
}

impl LocalApic {
    pub const fn new() -> LocalApic {
        let msr = Msr::new(IA32_APIC_BASE);

        LocalApic {
            msr,
            base: 0xFEE00000,
        }
    }

    pub fn init(&mut self) {
        let value = unsafe { self.msr.read() };
        self.base = value & 0xFFFFFF000;

        self.write(LocalApic::SIV, self.read(LocalApic::SIV) | 0x1FF);

        self.write(LocalApic::LVT_TIMER, 60 | LocalApic::TIMER_PERIODIC);
        self.write(LocalApic::DCR_TIMER, 3);

        self.write(LocalApic::INITCNT_TIMER, 1000000);
    }

    const ID: u16 = 0x20;
    const VERSION: u16 = 0x30;
    const TPR: u16 = 0x80;
    const APR: u16 = 0x90;
    const PPR: u16 = 0xA0;
    const EOI: u16 = 0xB0;
    const RRD: u16 = 0xC0;
    const LDR: u16 = 0xD0;
    const DFR: u16 = 0xE0;
    const SIV: u16 = 0xF0;

    const ERROR_STATUS: u16 = 0x280;

    const LVT_TIMER: u16 = 0x320;
    const LVT_THERMAL: u16 = 0x330;
    const LVT_PMC: u16 = 0x340;
    const LVT_LINT0: u16 = 0x350;
    const LVT_LINT1: u16 = 0x360;
    const LVT_ERROR: u16 = 0x370;
    const INITCNT_TIMER: u16 = 0x380;
    const CURCNT_TIMER: u16 = 0x390;
    const DCR_TIMER: u16 = 0x3E0;

    const TIMER_PERIODIC: u32 = 0x20000;

    fn write(&mut self, offset: u16, value: u32) {
        unsafe {
            core::ptr::write_volatile((self.base + offset as u64) as *mut _, value);
        }
    }

    fn read(&self, offset: u16) -> u32 {
        unsafe { core::ptr::read_volatile((self.base + offset as u64) as *mut _) }
    }

    pub fn send_eoi(&mut self) {
        self.write(LocalApic::EOI, 0);
    }
}

pub struct RedirectionEntry {
    low: u32,
    high: u32,
}

impl RedirectionEntry {
    pub fn new() -> RedirectionEntry {
        RedirectionEntry { low: 0, high: 0 }
    }

    #[inline]
    pub fn set_vector(&mut self, vector: u8) {
        self.low.set_bits(0..=7, vector as _);
    }

    #[inline]
    pub fn set_delivery_mode(&mut self, mode: u8) {
        self.low.set_bits(8..=10, mode as _);
    }

    #[inline]
    pub fn set_destination_mode(&mut self, mode: u8) {
        self.low.set_bit(11, mode == 1);
    }

    #[inline]
    pub fn set_delivery_status(&mut self, status: u8) {
        self.low.set_bit(12, status == 1);
    }

    #[inline]
    pub fn set_polarity(&mut self, polarity: u8) {
        self.low.set_bit(13, polarity == 1);
    }

    #[inline]
    pub fn set_IRR(&mut self, irr: u8) {
        self.low.set_bit(14, irr == 1);
    }

    #[inline]
    pub fn set_trigger_mode(&mut self, mode: u8) {
        self.low.set_bit(15, mode == 1);
    }

    #[inline]
    pub fn set_mask(&mut self, mask: u8) {
        self.low.set_bit(16, mask == 1);
    }

    #[inline]
    pub fn set_destination(&mut self, dest: u8) {
        self.high.set_bits(24..=31, dest as _);
    }

    #[inline]
    pub fn enable(&mut self) {
        self.set_mask(1);
    }

    #[inline]
    pub fn disable(&mut self) {
        self.set_mask(0);
    }

    #[inline]
    pub fn get_low(&self) -> u32 {
        self.low
    }

    #[inline]
    pub fn get_high(&self) -> u32 {
        self.high
    }
}

pub struct IOApic {
    base: u64,
}

impl IOApic {
    const ID: u16 = 0;
    const VERSION: u16 = 1;
    const ARB: u16 = 2;
    const RED_TABLE: u16 = 0x10;

    pub const fn new() -> IOApic {
        IOApic { base: 0 }
    }

    pub fn init(&mut self) {
        let xsdt = get_xsdt();
        let madt = xsdt
            .iter()
            .find(|table| table.signature == Signature::MADT.as_bytes())
            .expect("Unable to get MADT!");

        let ioapic = madt
            .get_entry::<madt::MADT>()
            .iter()
            .find(|e| {
                if let Entry::IoApic { .. } = e {
                    true
                } else {
                    false
                }
            })
            .expect("Unable to find Local Apic in madt!");

        match ioapic {
            Entry::IoApic {
                io_apic_address, ..
            } => self.base = *io_apic_address as u64,
            _ => (),
        }

        let mut re = RedirectionEntry::new();
        re.set_vector(0x45);
        self.write_entry(1, &re);
    }

    pub fn write(&mut self, offset: u16, value: u32) {
        unsafe {
            core::ptr::write_volatile(self.base as *mut u32, offset as _); // IOREGSEL
            core::ptr::write_volatile((self.base + 0x10) as *mut u32, value); // IOWIN
        }
    }

    pub fn read(&self, offset: u16) -> u32 {
        unsafe {
            core::ptr::write_volatile(self.base as *mut u32, offset as _); // IOREGSEL
            core::ptr::read_volatile((self.base + 10) as *mut u32) // IOWIN
        }
    }

    pub fn write_entry(&mut self, vector: u8, entry: &RedirectionEntry) {
        let address = IOApic::RED_TABLE + (2 * vector as u16);
        self.write(address, entry.get_low());
        self.write(address + 1, entry.get_high());
    }
}
