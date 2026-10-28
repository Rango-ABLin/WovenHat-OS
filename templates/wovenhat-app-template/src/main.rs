#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    libwoven::exit(0)
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    libwoven::exit(101)
}
