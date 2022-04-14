#![cfg_attr(not(test), no_std)]

pub trait DriverCore {
    #[no_mangle]
    fn init();

    #[no_mangle]
    fn uninit();
}

#[macro_export]
macro_rules! driver {
    () => {
        use core::panic::PanicInfo;

        #[panic_handler]
        fn panic(_: &PanicInfo) -> ! {
            loop {}
        }
    };
}

pub use driver_proc_macro::*;