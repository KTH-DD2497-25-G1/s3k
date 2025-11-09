#![no_std]
#![feature(linkage)]

mod console;
mod ffi;
mod syscall;

use core::arch::naked_asm;
use core::panic::PanicInfo;

unsafe extern "C" {
    fn __stack_pointer();
}

#[unsafe(naked)]
unsafe extern "C" fn prepare() {
    naked_asm!("la sp, {}", sym __stack_pointer);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".init")]
pub extern "C" fn _start() {
    loop {
        unsafe { prepare() };
        main();
    }
}

#[unsafe(no_mangle)]
#[linkage = "weak"]
fn main() {
    panic!("Cannot find main!");
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    // println!("Panic: {}", info);
    loop {}
}
