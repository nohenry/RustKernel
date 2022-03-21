mod types;
use core::sync::atomic::AtomicPtr;

pub use types::*;

use crate::efi::{GLOBAL_SYSTEM_TABLE, guid};

const RSDP: AtomicPtr<RSDP> = AtomicPtr::new(core::ptr::null_mut());

pub fn init() {
    let efi_table = unsafe {&*GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst)};

    let mut tables = efi_table.config_tables();
    let tbl = tables.find(|e| {kprintln!("{}", e.0); e.0 == guid::RSDP}).expect("Unable to find RSDP!");
    
    let rsdp = tbl.1 as *mut RSDP;
    kprintln!("{:p}", rsdp);
    RSDP.compare_exchange(
        core::ptr::null_mut(),
        rsdp,
        core::sync::atomic::Ordering::SeqCst,
        core::sync::atomic::Ordering::SeqCst,
    );

    let rsdp = unsafe { &*rsdp };
    let xsdt = unsafe { &*rsdp.xsdt };

    for table in xsdt.iter() {
    //     table.signature();
    //     // kprintln!("{}", table.signature);
    }
}

pub fn get_xsdt() -> &'static XSDT {
    let rsdp = unsafe {&* RSDP.load(core::sync::atomic::Ordering::SeqCst) };
    unsafe { &*rsdp.xsdt }
}