#![no_std]
#![no_main]

mod device;
mod fat32;
mod ffi;
mod file;
mod inode;
mod logger;
mod result;
mod time;
mod utils;

extern crate alloc;

use alloc::sync::Arc;
use log::{debug, error, info};
use s3k_common::ffi::{S3kErr, S3kMemPerm, S3kReg};
use s3k_common::plat::UART0_BASE_ADDR;
use s3k_common::heap;
use s3k_common::syscall::{
    s3k_cap_derive, s3k_mon_cap_move, s3k_mon_pmp_load, s3k_mon_reg_write, s3k_pmp_load,
    s3k_sync_mem,
};
use s3k_common::utils::*;
use crate::device::virtio::VirtIOBlkDevice;
use crate::fat32::FAT32FileSystem;

type Result<T> = core::result::Result<T, S3kErr>;

fn setup_other_app() -> Result<()> {
    let uart_addr = s3k_napot_encode(UART0_BASE_ADDR, 0x8);
    let app1_addr = s3k_napot_encode(APP_1_BASE_ADDR, APP_1_SIZE);

    // Derive a PMP capability for app1 main memory
    let free_cap_mem_idx = find_free_cap()?;
    s3k_cap_derive(
        RAM_MEM,
        free_cap_mem_idx,
        s3k_mk_memory(
            APP_1_BASE_ADDR,
            APP_1_BASE_ADDR + APP_1_SIZE,
            S3kMemPerm::RWX,
        ),
    )?;
    let free_cap_idx = find_free_cap()?;
    s3k_cap_derive(
        free_cap_mem_idx,
        free_cap_idx,
        s3k_mk_pmp(app1_addr, S3kMemPerm::RWX),
    )?;
    s3k_mon_cap_move(MONITOR, APP0_PID, free_cap_idx, APP1_PID, APP_1_CAP_PMP_MEM)?;
    s3k_mon_pmp_load(MONITOR, APP1_PID, APP_1_CAP_PMP_MEM, APP_1_PMP_SLOT_MEM)?;

    // Keep a PMP for ourselves to reqrite APP1_memory
    let free_cap_idx = find_free_cap()?;
    s3k_cap_derive(
        free_cap_mem_idx,
        free_cap_idx,
        s3k_mk_pmp(app1_addr, S3kMemPerm::RWX),
    )?;
    s3k_pmp_load(free_cap_idx, BUFFER_PMP)?;

    // Derive a PMP capability for uart
    let free_cap_idx = find_free_cap()?;
    s3k_cap_derive(
        UART_MEM,
        free_cap_idx,
        s3k_mk_pmp(uart_addr, S3kMemPerm::RW),
    )?;
    s3k_mon_cap_move(
        MONITOR,
        APP0_PID,
        free_cap_idx,
        APP1_PID,
        APP_1_CAP_PMP_UART,
    )?;
    s3k_mon_pmp_load(MONITOR, APP1_PID, APP_1_CAP_PMP_UART, APP_1_PMP_SLOT_UART)?;

    // Write start PC of app1 to PC
    s3k_mon_reg_write(MONITOR, APP1_PID, S3kReg::PC, APP_1_BASE_ADDR as u64)?;

    s3k_sync_mem();

    Ok(())
}

fn _main() -> Result<()> {
    setup_uart_and_virtio()?;
    heap::init();
    logger::init();
    info!("fsd start");

    let device = Arc::new(VirtIOBlkDevice::new());
    info!("virtio block device created");

    let fs = match FAT32FileSystem::new(device) {
        Ok(fs) => fs,
        Err(e) => {
            error!("Failed to mount FAT32 filesystem: {:?}", e);
            return Err(S3kErr::Unknown);
        }
    };
    info!("filesystem mounted");

    let root = fs.root();
    let mut idx = 0;
    while let Ok(inode) = root.clone().lookup_idx(idx) {
        debug!("List file {}: {}", idx, inode.metadata().path);
        idx += 1;
    }

    // Setup app1 capabilities and PC
    // setup_other_app()?;

    Ok(())
}

#[unsafe(no_mangle)]
fn main() {
    if let Err(e) = _main() {
        panic!("fsd failed with: {:?}", e);
    }
}
