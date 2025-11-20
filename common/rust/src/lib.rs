#![no_std]
#![feature(linkage)]

pub mod console;
pub mod driver;
pub mod ffi;
pub mod plat;
pub mod syscall;

use core::arch::asm;
use core::panic::PanicInfo;

unsafe extern {
    fn __stack_pointer();
    fn _bss();
    fn _end();
}

unsafe fn clear_bss() {
    let bss_start = _bss as usize;
    let bss_end = _end as usize;
    core::slice::from_raw_parts_mut(bss_start as _, bss_end - bss_start).fill(0);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.init")]
unsafe extern "C" fn _start() {
    loop {
        unsafe { asm!("la sp, {}", sym __stack_pointer); }
        clear_bss();
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
