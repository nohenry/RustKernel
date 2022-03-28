// Screw you gdt

use lazy_static::lazy_static;
use x86_64::{
    instructions::{segmentation, tables},
    registers::segmentation::Segment,
    structures::gdt,
    structures::tss::TaskStateSegment,
    VirtAddr,
};

use crate::mem::STACK_SIZE;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

struct Selectors {
    code_selector: gdt::SegmentSelector,
    data_selector: gdt::SegmentSelector,
    tss_selector: gdt::SegmentSelector,
}
static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            kprintln!("STACK {:p}", unsafe { &STACK });

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (gdt::GlobalDescriptorTable, Selectors) = {
        let mut gdt = gdt::GlobalDescriptorTable::new();
        let kcode = gdt.add_entry(gdt::Descriptor::kernel_code_segment());
        let kdata = gdt.add_entry(gdt::Descriptor::kernel_data_segment());
        gdt.add_entry(gdt::Descriptor::UserSegment(0));
        gdt.add_entry(gdt::Descriptor::user_data_segment());
        gdt.add_entry(gdt::Descriptor::user_code_segment());

        let tss = gdt.add_entry(gdt::Descriptor::tss_segment(&TSS));
        (
            gdt,
            Selectors {
                code_selector: kcode,
                data_selector: kdata,
                tss_selector: tss,
            },
        )
    };
}

pub fn init() {
    GDT.0.load();
    unsafe {
        segmentation::CS::set_reg(GDT.1.code_selector);
        segmentation::DS::set_reg(GDT.1.data_selector);
        segmentation::ES::set_reg(GDT.1.data_selector);
        segmentation::FS::set_reg(GDT.1.data_selector);
        segmentation::GS::set_reg(GDT.1.data_selector);
        segmentation::SS::set_reg(GDT.1.data_selector);
        tables::load_tss(GDT.1.tss_selector);
    }
}
