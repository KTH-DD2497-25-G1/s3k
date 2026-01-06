use core::fmt::{self, Write};

#[cfg(any(feature = "qemu_virt", feature = "qemu_virt4"))]
use crate::plat::get_uart;

struct UartWriter;

#[cfg(any(feature = "qemu_virt", feature = "qemu_virt4"))]
impl Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            get_uart().putchar(byte);
        }
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    let _ = UartWriter.write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($fmt: expr $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?));
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: expr $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}

#[cfg(any(feature = "qemu_virt", feature = "qemu_virt4"))]
pub fn getchar() -> u8 {
    get_uart().getchar()
}


#[cfg(any(feature = "qemu_virt", feature = "qemu_virt4"))]
pub fn read_line(buffer: &mut [u8]) -> usize {
    let mut index = 0;

    loop {
        let c = getchar();

        match c {
            b'\r' | b'\n' => {
                print!("\n");
                break;
            }

            b'\x08' | b'\x7f' => {
                if index > 0 {
                    index -= 1;
                    // Visual Backspace: Move back, overwrite with space, move back again
                    print!("{}", '\x08'); // Move cursor left
                    print!("{}", ' ');    // Overwrite char with space
                    print!("{}", '\x08'); // Move cursor left again
                }
            }

            c => {
                if index < buffer.len() {
                    buffer[index] = c;
                    index += 1;
                    print!("{}", c as char);
                }
            }
        }
    }
    index
}