use crate::ffi::*;
use crate::plat::UART0_BASE_ADDR;
use crate::syscall::{s3k_cap_derive, s3k_cap_read, s3k_pmp_load, s3k_sync_mem};

pub static APP0_PID: S3kPid = 0;
pub static APP1_PID: S3kPid = 1;

// See plat_conf.h
pub static BOOT_PMP: S3kCidx = 0;
pub static RAM_MEM: S3kCidx = 1;
pub static UART_MEM: S3kCidx = 2;
pub static TIME_MEM: S3kCidx = 3;
pub static HART0_TIME: S3kCidx = 4;
pub static HART1_TIME: S3kCidx = 5;
pub static HART2_TIME: S3kCidx = 6;
pub static HART3_TIME: S3kCidx = 7;
pub static MONITOR: S3kCidx = 8;
pub static CHANNEL: S3kCidx = 9;

// normal caps
pub static UART_CAP: S3kCidx = 10;
pub static UART_PMP: S3kPmpSlot = 1;
pub static BUFFER_PMP: S3kPmpSlot = 2;

// CAPS for second process
pub static APP_1_CAP_PMP_MEM: S3kCidx = 0;
pub static APP_1_CAP_PMP_UART: S3kCidx = 10;
pub static APP_1_TIME: S3kCidx = 1;
pub static APP_1_CAP_SOCKET: S3kCidx = 3;

pub static APP_1_PMP_SLOT_MEM: S3kPmpSlot = 0;
pub static APP_1_PMP_SLOT_UART: S3kPmpSlot = 1;
pub static APP_1_PMP_SLOT_BUFFER: S3kPmpSlot = 2;

// Other constants
pub static APP_1_BASE_ADDR: usize = 0x80110000;
pub static APP_1_SIZE: usize = 0x10000;
pub static SHARED_BUFFER_BASE: usize = APP_1_BASE_ADDR + APP_1_SIZE;
pub static SHARED_BUFFER_SIZE: usize = 0x10000;

static FREE_CAP_BEGIN: u16 = 12;
static FREE_CAP_END: u16 = 20;

pub fn setup_uart_and_virtio() -> Result<(), S3kErr> {
    let uart_addr = s3k_napot_encode(UART0_BASE_ADDR, 0x2000);
    // Derive a PMP capability for accessing UART
    s3k_cap_derive(UART_MEM, UART_CAP, s3k_mk_pmp(uart_addr, S3kMemPerm::RW))?;
    // Load the derive PMP capability to PMP configuration
    s3k_pmp_load(UART_CAP, UART_PMP)?;
    // Synchronize PMP unit (hardware) with PMP configuration
    // false => not full synchronization.
    s3k_sync_mem();
    Ok(())
}

pub fn find_free_cap() -> Result<S3kCidx, S3kErr> {
    for i in FREE_CAP_BEGIN..FREE_CAP_END {
        match s3k_cap_read::<EmptyCap>(i) {
            Ok(_) => continue,
            Err(e) => {
                if e == S3kErr::Empty { return Ok(i); }
                continue
            },
        }
    }
    Err(S3kErr::InvalidCapability)
}

pub fn s3k_napot_encode(base: S3kAddr, size: usize) -> S3kNapot {
    ((base | (size / 2 - 1)) >> 2) as u64
}

pub fn s3k_mk_memory(bgn: usize, end: usize, rwx: S3kMemPerm) -> MemCap {
    let tag = bgn >> S3K_MAX_BLOCK_SIZE;
    let bgn_block = ((bgn - (tag << S3K_MAX_BLOCK_SIZE)) >> S3K_MIN_BLOCK_SIZE) as u16;
    let end_block = ((end - (tag << S3K_MAX_BLOCK_SIZE)) >> S3K_MIN_BLOCK_SIZE) as u16;
    MemCap::new()
        .with_tag(tag as u8)
        .with_bgn(bgn_block)
        .with_end(end_block)
        .with_mrk(bgn_block)
        .with_rwx(rwx.bits())
        .with_lck(false)
}

pub fn s3k_mk_pmp(addr: S3kNapot, rwx: S3kMemPerm) -> PmpCap {
    PmpCap::new()
        .with_addr(addr)
        .with_rwx(rwx.bits())
        .with_used(false)
        .with_slot(0)
}

pub fn s3k_mk_socket(chan: S3kChan, mode: S3kIpcMode, perm: S3kIpcPerm, tag: u32) -> SockCap {
    SockCap::new()
        .with_chan(chan)
        .with_mode(mode)
        .with_perm(perm.bits())
        .with_tag(tag)
}