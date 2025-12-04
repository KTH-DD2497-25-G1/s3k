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
