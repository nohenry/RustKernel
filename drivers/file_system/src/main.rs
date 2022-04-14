#![no_std]
#![no_main]

use driver::server_entry;

#[server_entry]
fn main() {

}

driver::driver!();