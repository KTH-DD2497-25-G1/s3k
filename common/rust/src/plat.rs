use crate::driver::uart::ns16550a::UartDevice;

#[cfg(any(feature = "qemu_virt", feature = "qemu_virt4"))]
const UART0_BASE_ADDR: usize = 0x10000000;

#[cfg(any(feature = "qemu_virt", feature = "qemu_virt4"))]
pub fn get_uart() -> &'static UartDevice {
    static mut UART0: Option<UartDevice> = None;
    unsafe {
        #[allow(static_mut_refs)]
        UART0.get_or_insert_with(|| UartDevice::new(UART0_BASE_ADDR))
    }
}
