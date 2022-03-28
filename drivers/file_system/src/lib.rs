#![no_std]
use driver::DriverCore;

struct FileSystem;

impl DriverCore for FileSystem {
    fn init() {}

    fn uninit() {}
}

driver::driver!();
