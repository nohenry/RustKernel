mod types;
use core::sync::atomic::AtomicPtr;

pub use types::*;

use crate::efi::{GLOBAL_SYSTEM_TABLE, guid};

const RSDP: AtomicPtr<types::RSDP> = AtomicPtr::new(core::ptr::null_mut());

pub fn init() {
    let efi_table = unsafe {&*GLOBAL_SYSTEM_TABLE.load(core::sync::atomic::Ordering::SeqCst)};

    let mut tables = efi_table.config_tables();
    let tbl = tables.find(|e| e.0 == guid::RSDP).expect("Unable to find RSDP!");
    
}