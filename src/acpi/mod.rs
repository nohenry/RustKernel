
mod types;
pub mod xsdt;
pub mod madt;
pub mod fadt;
pub mod mcfg;
pub mod aml;

use core::sync::atomic::AtomicPtr;
use xsdt::XSDT;
use crate::efi::{GLOBAL_SYSTEM_TABLE, guid};

pub use types::*;

pub static RSDP: AtomicPtr<RSDP> = AtomicPtr::new(core::ptr::null_mut());

pub fn init() {
    let efi_table = unsafe {&*GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst)};

    let mut tables = efi_table.config_tables();
    let tbl = tables.find(|e| {kprintln!("{}", e.0); e.0 == guid::RSDP}).expect("Unable to find RSDP!");
    
    let rsdp = tbl.1 as *mut RSDP;
    RSDP.compare_exchange(
        core::ptr::null_mut(),
        rsdp,
        core::sync::atomic::Ordering::SeqCst,
        core::sync::atomic::Ordering::SeqCst,
    ).expect("Unable to set value!");

    let rsdp = unsafe { &*rsdp };
    let xsdt = unsafe { &*rsdp.xsdt };

    for table in xsdt.iter() {
        if table.signature == Signature::MADT.as_bytes() {
            let madt = table.get_entry::<madt::MADT>();
            for entry in madt.iter() {
                kprintln!("{:?}", entry);
            }
        }
        kprintln!("ACPI: {}", table.signature());
    }
}

pub fn get_xsdt() -> &'static XSDT {
    let rsdp = unsafe {&* RSDP.load(core::sync::atomic::Ordering::SeqCst) };
    unsafe { &*rsdp.xsdt }
}