#![no_std]
#![feature(linkage)]

mod console;
mod driver;
mod ffi;
mod plat;
mod syscall;

use core::arch::asm;
use core::panic::PanicInfo;

unsafe extern "C" {
    fn __stack_pointer();
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.init")]
pub unsafe extern "C" fn _start() {
    loop {
        unsafe { asm!("la sp, {}", sym __stack_pointer); }
        main();
    }
}

#[linkage = "weak"]
fn main() {
    panic!("Cannot find main!");
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    println!("Panic: {}", info);
    loop {}
}
