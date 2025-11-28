#![no_std]
#![feature(linkage)]

pub mod console;
pub mod driver;
pub mod ffi;
pub mod plat;
pub mod syscall;
pub mod utils;

use core::arch::naked_asm;
use core::panic::PanicInfo;

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.init")]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "la sp, __stack_pointer",
        "la t0, _bss",
        "la t1, _end",
        "beq t0, t1, 1f",
        "0:",
        "sb zero, 0(t0)",
        "addi t0, t0, 1",
        "bne t0, t1, 0b",
        "1:",
        "call main",
        "j .",
    )
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    println!("Panic: {}", info);
    loop {}
}
