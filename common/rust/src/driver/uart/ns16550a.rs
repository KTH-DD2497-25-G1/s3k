const CTRL_C: u8 = 3;

pub struct UartDevice(usize);

trait AsPtr {
    fn as_ptr(&self) -> *mut u8;
}

impl AsPtr for usize {
    fn as_ptr(&self) -> *mut u8 {
        *self as *mut u8
    }
}

impl UartDevice {
    fn rxdata_ptr(&self) -> *mut u8 {
        self.0.as_ptr()
    }

    fn txdata_ptr(&self) -> *mut u8 {
        self.0.as_ptr()
    }

    fn ie_ptr(&self) -> *mut u8 {
        (self.0 + 1).as_ptr()
    }

    fn fifo_ctrl_ptr(&self) -> *mut u8 {
        (self.0 + 2).as_ptr()
    }

    fn is_ptr(&self) -> *mut u8 {
        (self.0 + 2).as_ptr()
    }

    fn line_ctrl_ptr(&self) -> *mut u8 {
        (self.0 + 3).as_ptr()
    }

    fn line_status_ptr(&self) -> *mut u8 {
        (self.0 + 5).as_ptr()
    }
}

impl UartDevice {
    pub fn new(base_addr: usize) -> Self {
        let dev = UartDevice(base_addr);
        unsafe {
            dev.ie_ptr().write_volatile(0);
            dev.fifo_ctrl_ptr().write_volatile((1 << 0) | (3 << 1));
            dev.ie_ptr().write_volatile(1);
        }
        dev
    }

    pub fn has_data(&self) -> bool {
        unsafe { self.line_status_ptr().read_volatile() & 0x01 == 0x01 }
    }

    pub fn getchar(&self) -> u8 {
        while !self.has_data() {}
        unsafe { self.rxdata_ptr().read_volatile() }
    }

    pub fn putchar(&self, ch: u8) {
        unsafe {
            while (self.line_status_ptr().read_volatile() & (1 << 5)) == 0 {}
            self.txdata_ptr().write_volatile(ch);
        }
    }
}
