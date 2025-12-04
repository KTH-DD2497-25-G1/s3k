use bitflags::bitflags;

pub static PAGE_SIZE: usize = 4096;

pub type Uid = u16;
pub type Gid = u16;

bitflags! {
    #[derive(Copy, Clone, Eq, PartialEq)]
    pub struct InodeMode: u32 {
        const S_IFIFO  = 0x1000;
        const S_IFCHR  = 0x2000;
        const S_IFDIR  = 0x4000;
        const S_IFBLK  = 0x6000;
        const S_IFREG  = 0x8000;
        const S_IFLNK  = 0xA000;
        const S_IFSOCK = 0xC000;
        const S_IFMT   = 0xF000;
        const S_ISUID  = 0x0800;
        const S_ISGID  = 0x0400;
        const S_ISVTX  = 0x0200;
        const S_IRUSR  = 0x0100;
        const S_IWUSR  = 0x0080;
        const S_IXUSR  = 0x0040;
        const S_IRWXU  = Self::S_IRUSR.bits() | Self::S_IWUSR.bits() | Self::S_IXUSR.bits();
        const S_IRGRP  = 0x0020;
        const S_IWGRP  = 0x0010;
        const S_IXGRP  = 0x0008;
        const S_IRWXG  = Self::S_IRGRP.bits() | Self::S_IWGRP.bits() | Self::S_IXGRP.bits();
        const S_IROTH  = 0x0004;
        const S_IWOTH  = 0x0002;
        const S_IXOTH  = 0x0001;
        const S_IRWXO  = Self::S_IROTH.bits() | Self::S_IWOTH.bits() | Self::S_IXOTH.bits();
        const S_ACCESS = Self::S_IRWXU.bits() | Self::S_IRWXG.bits() | Self::S_IRWXO.bits();
        const S_MISC   = Self::S_ACCESS.bits() | Self::S_ISUID.bits() | Self::S_ISGID.bits() | Self::S_ISVTX.bits();
    }
}

impl InodeMode {
    pub fn def_dir() -> Self {
        InodeMode::S_IFDIR | InodeMode::from_bits(0o777).unwrap()
    }

    pub fn def_file() -> Self {
        InodeMode::S_IFREG | InodeMode::from_bits(0o666).unwrap()
    }

    pub fn def_lnk() -> Self {
        InodeMode::S_IFLNK | InodeMode::from_bits(0o777).unwrap()
    }

    pub fn from_bits_access(bits: u32) -> Self {
        Self::from_bits_retain(bits) & InodeMode::S_ACCESS
    }

    pub fn from_bits_misc(bits: u32) -> Self {
        Self::from_bits_retain(bits) & InodeMode::S_MISC
    }

    pub fn file_type(&self) -> InodeMode {
        *self & InodeMode::S_IFMT
    }

    pub fn is_dir(&self) -> bool {
        self.file_type() == InodeMode::S_IFDIR
    }

    pub fn is_reg(&self) -> bool {
        self.file_type() == InodeMode::S_IFREG
    }

    pub fn is_lnk(&self) -> bool {
        self.file_type() == InodeMode::S_IFLNK
    }
}

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
