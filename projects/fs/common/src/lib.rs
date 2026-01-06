#![no_std]
use bitflags::bitflags;

pub mod logger;
pub const REQ_OPEN: u64 = 1;
pub const REQ_CLOSE: u64 = 2;
pub const REQ_READ: u64 = 3;
pub const REQ_WRITE: u64 = 4;
pub const REQ_SEEK: u64 = 5;
pub const REQ_LS: u64 = 6;
pub const REQ_SIZE: u64 = 7;
pub const REQ_MKDIR: u64 = 8;

pub const REQ_ENABLE_ENCRYPTION: u64 = 20;
pub const REQ_DISABLE_ENCRYPTION: u64 = 21;
pub const REQ_UNLOCK_DEVICE: u64 = 22;

bitflags! {
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct OpenFlags: u32 {
        const O_RDONLY    =        0o0;
        const O_WRONLY    =        0o1;
        const O_RDWR      =        0o2;
        const O_CREAT     =      0o100;
        const O_EXCL      =      0o200;
        const O_NOCTTY    =      0o400;
        const O_TRUNC     =     0o1000;
        const O_APPEND    =     0o2000;
        const O_NONBLOCK  =     0o4000;
        const O_DSYNC     =    0o10000;
        const O_ASYNC     =    0o20000;
        const O_DIRECT    =    0o40000;
        const O_LARGEFILE =   0o100000;
        const O_DIRECTORY =   0o200000;
        const O_NOFOLLOW  =   0o400000;
        const O_NOATIME   =  0o1000000;
        const O_CLOEXEC   =  0o2000000;
        const O_SYNC      =  0o4010000;
        const O_PATH      = 0o10000000;
        const O_STATUS    = Self::O_APPEND.bits() | Self::O_ASYNC.bits() | Self::O_DIRECT.bits() | Self::O_NOATIME.bits() | Self::O_NONBLOCK.bits();
    }
}

impl OpenFlags {
    pub fn readable(&self) -> bool {
        self.contains(OpenFlags::O_RDONLY) || self.contains(OpenFlags::O_RDWR)
    }

    pub fn writable(&self) -> bool {
        self.contains(OpenFlags::O_WRONLY) || self.contains(OpenFlags::O_RDWR)
    }
}
