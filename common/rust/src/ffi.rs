use bitfield_struct::bitfield;
use enumflags2::{BitFlags, bitflags};
use num_enum::FromPrimitive;

macro_rules! s3k_type {
    ($name:ident, $ty:ty) => {
        #[derive(Debug, Clone, Copy)]
        #[repr(transparent)]
        pub struct $name(pub $ty);

        impl $name {
            const fn from_bits(bits: $ty) -> Self {
                Self(bits)
            }
            const fn into_bits(self) -> $ty {
                self.0
            }
        }
    };
}

s3k_type!(S3kNapot, u64);
s3k_type!(S3kAddr, u64);
s3k_type!(S3kState, u64);
s3k_type!(S3kBlock, u16);
s3k_type!(S3kChan, u16);
s3k_type!(S3kTimeSlot, u16);
s3k_type!(S3kPid, u16);
s3k_type!(S3kCidx, u16);
s3k_type!(S3kHart, u8);
s3k_type!(S3kTag, u8);
s3k_type!(S3kRwx, u8);
s3k_type!(S3kPmpSlot, u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[repr(u8)]
pub enum S3kErr {
    Success = 0,
    Empty,
    SrcEmpty,
    DstOccupied,
    InvalidIndex,
    InvalidDerivation,
    InvalidMonitor,
    InvalidPid,
    InvalidState,
    InvalidPmp,
    InvalidSlot,
    InvalidSocket,
    InvalidSyscall,
    InvalidRegister,
    InvalidCapability,
    NoReceiver,
    Preempted,
    Timeout,
    Suspended,
    #[num_enum(default)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum S3kIpcMode {
    NOYIELD = 0,
    YIELD = 1,
}

impl S3kIpcMode {
    const fn from_bits(bits: u8) -> Self {
        unsafe { core::mem::transmute(bits) }
    }
    const fn into_bits(self) -> u8 {
        self as u8
    }
}

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum S3kIpcPerm {
    SDATA = 0x1,
    SCAP = 0x2,
    CDATA = 0x4,
    CCAP = 0x8,
}
pub type S3kIpcPerms = BitFlags<S3kIpcPerm>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum CapType {
    /// No capability.
    None = 0,
    /// Time Slice capability.
    Time = 1,
    /// Memory Slice capability.
    Memory = 2,
    /// PMP Frame capability.
    Pmp = 3,
    /// Monitor capability.
    Monitor = 4,
    /// IPC Channel capability.
    Channel = 5,
    /// IPC Socket capability.
    Socket = 6,
}

impl CapType {
    const fn from_bits(bits: u8) -> Self {
        match bits {
            0 => CapType::None,
            1 => CapType::Time,
            2 => CapType::Memory,
            3 => CapType::Pmp,
            4 => CapType::Monitor,
            5 => CapType::Channel,
            6 => CapType::Socket,
            _ => CapType::None,
        }
    }

    const fn into_bits(self) -> u8 {
        self as u8
    }
}

pub trait S3kCap: Sized {
    unsafe fn from_raw(raw: u64) -> Result<Self, S3kErr>;
}

macro_rules! impl_cap {
    ($name:ident, $ty:expr) => {
        impl S3kCap for $name {
            unsafe fn from_raw(raw: u64) -> Result<Self, S3kErr> {
                let cap: Self = core::mem::transmute(raw);
                if cap.ty() == $ty {
                    Ok(cap)
                } else {
                    Err(S3kErr::InvalidCapability)
                }
            }
        }
    }
}

#[bitfield(u64)]
pub struct TimeCap {
    #[bits(4, default = CapType::Time)]
    ty: CapType,
    #[bits(4)]
    _padding: u8,
    #[bits(8)]
    pub hart: S3kHart,
    #[bits(16)]
    pub bgn: S3kTimeSlot,
    #[bits(16)]
    pub mrk: S3kTimeSlot,
    #[bits(16)]
    pub end: S3kTimeSlot,
}
impl_cap!(TimeCap, CapType::Time);

#[bitfield(u64)]
pub struct MemCap {
    #[bits(4, default = CapType::Memory)]
    ty: CapType,
    #[bits(3)]
    pub rwx: S3kRwx,
    pub lck: bool,
    #[bits(8)]
    pub tag: S3kTag,
    #[bits(16)]
    pub bgn: S3kBlock,
    #[bits(16)]
    pub mrk: S3kBlock,
    #[bits(16)]
    pub end: S3kBlock,
}
impl_cap!(MemCap, CapType::Memory);

#[bitfield(u64)]
pub struct PmpCap {
    #[bits(4, default = CapType::Pmp)]
    ty: CapType,
    #[bits(3)]
    pub rwx: S3kRwx,
    pub used: bool,
    #[bits(8)]
    pub pmp_slot: S3kPmpSlot,
    #[bits(16)]
    pub napot: S3kNapot,
    _reserved: u32,
}
impl_cap!(PmpCap, CapType::Pmp);

#[bitfield(u64)]
pub struct MonCap {
    #[bits(4, default = CapType::Monitor)]
    ty: CapType,
    #[bits(12)]
    _padding: u16,
    #[bits(16)]
    pub bgn: S3kPid,
    #[bits(16)]
    pub mrk: S3kPid,
    #[bits(16)]
    pub end: S3kPid,
}
impl_cap!(MonCap, CapType::Monitor);

#[bitfield(u64)]
pub struct ChanCap {
    #[bits(4, default = CapType::Channel)]
    ty: CapType,
    #[bits(12)]
    _padding: u16,
    #[bits(16)]
    pub bgn: S3kChan,
    #[bits(16)]
    pub mrk: S3kChan,
    #[bits(16)]
    pub end: S3kChan,
}
impl_cap!(ChanCap, CapType::Channel);

#[bitfield(u64)]
pub struct SockCap {
    #[bits(4, default = CapType::Socket)]
    ty: CapType,
    #[bits(4)]
    pub mode: S3kIpcMode,
    #[bits(8)]
    perm: u8,
    #[bits(16)]
    pub chan: S3kChan,
    tag: u32,
}
impl_cap!(SockCap, CapType::Socket);

impl SockCap {
    pub fn get_perms(&self) -> S3kIpcPerms {
        BitFlags::from_bits_truncate(self.perm())
    }

    pub fn set_perms(&mut self, perms: S3kIpcPerms) {
        self.set_perm(perms.bits())
    }
}

#[repr(C)]
pub struct S3kMsg {
    pub cap_idx: S3kCidx,
    pub send_cap: bool,
    pub data: [u64; 4],
}

#[repr(C)]
pub struct S3kReply {
    pub err: S3kErr,
    pub tag: u32,
    pub cap: u64,
    pub data: [u64; 4],
}

#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum S3kReg {
    PC,
    RA,
    SP,
    GP,
    TP,
    T0,
    T1,
    T2,
    S0,
    S1,
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
    S9,
    S10,
    S11,
    T3,
    T4,
    T5,
    T6,
    TPC,
    TSP,
    EPC,
    ESP,
    ECAUSE,
    EVAL,
    SERVTIME,
    /* Special value for number of registers */
    CNT,
}
