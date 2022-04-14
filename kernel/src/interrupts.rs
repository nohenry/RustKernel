use alloc::vec::Vec;
use bit_field::BitField;
use common::{
    kprint, kprintln,
    util::{in8, out8},
    x86_64::PhysAddr,
};
use core::{arch::asm, borrow::Borrow};
use macros::{generate_isrs, set_isrs};

use crate::{
    acpi::{
        get_xsdt,
        madt::{self, Entry},
        Signature, RSDP,
    },
    drivers::keyboard::Keyboard,
    gdt,
};

use common::process::{self, SYSCALL_SP, SYSCALL_UMAP, SYSCALL_USP};
use common::util;
use common::x86_64::{
    registers::model_specific::Msr,
    structures::idt::{self, InterruptDescriptorTable},
};
use lazy_static::lazy_static;

const IA32_APIC_BASE: u32 = 0x1b;

use spin;

pub static APIC: spin::Mutex<LocalApic> = spin::Mutex::new(LocalApic::new());
pub static IOAPIC: spin::Mutex<IOApic> = spin::Mutex::new(IOApic::new());
static mut HANDLERS: [Vec<fn(&mut InterruptStackFrame, &CpuSnapshot)>; 256 - 32] =
    [const { Vec::new() }; 256 - 32];

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

pub use idt::InterruptStackFrame;

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
    );
}
    };
}

#[macro_export]
macro_rules! interrupt_end {
    () => {
        unsafe {
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
        ", options(noreturn)
            );
        }
    };
    (sti) => {
        unsafe {
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

generate_isrs!();

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

        /* APIC Stuff */
        unsafe {
            set_isrs!(idt);
            idt[0xFF].set_handler_fn(lapic_spurious);
        }
        idt
    };
}

pub fn init() {
    common::x86_64::instructions::interrupts::disable();

    IDT.load();

    APIC.lock().init();
    IOAPIC.lock().init();

    unsafe {
        out8(0x21, 0xFF);
        out8(0xA1, 0xFF);
    }
}

pub fn register_handler(vector: u8, handler: fn(&mut InterruptStackFrame, &CpuSnapshot)) {
    unsafe {
        HANDLERS[vector as usize - 32].push(handler);
    }
}

fn interrupt(stack_frame: &mut idt::InterruptStackFrame, snapshot: &CpuSnapshot, vector: u8) {
    unsafe {
        for handler in &HANDLERS[vector as usize - 32] {
            (handler)(stack_frame, snapshot);
        }
    }
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
    use common::x86_64::registers::control::Cr2;

    kprintln!(
        "EXCPETION: PAGE FAULT\n{:#?}\n{:#?}\n",
        _stack_frame,
        _error_code
    );
    kprintln!("Address: {:?}\n", Cr2::read());

    loop {}
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

        common::mem::map_phys(PhysAddr::new(self.base), 4096);

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
