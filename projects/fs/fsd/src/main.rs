#![no_std]
#![no_main]

mod device;
mod fat32;
mod ffi;
mod file;
mod inode;
mod result;
mod time;
mod utils;

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use log::{debug, error, info, warn};
use s3k_common::ffi::{S3kCidx, S3kErr, S3kIpcMode, S3kIpcPerm, S3kMemPerm, S3kMsg, S3kReg, S3kReply};
use s3k_common::plat::UART0_BASE_ADDR;
use s3k_common::heap;
use s3k_common::syscall::{s3k_cap_delete, s3k_cap_derive, s3k_cap_revoke, s3k_mon_cap_move, s3k_mon_pmp_load, s3k_mon_reg_write, s3k_mon_resume, s3k_mon_yield, s3k_pmp_load, s3k_pmp_unload, s3k_reg_write, s3k_sock_recv, s3k_sock_send, s3k_sync, s3k_sync_mem};
use s3k_common::utils::*;
use fs_common::OpenFlags;
use spin::Mutex;
use crate::device::virtio::VirtIOBlkDevice;
use crate::fat32::FAT32FileSystem;
use crate::ffi::{InodeMode};
use crate::file::{DirFile, File, FileMeta, RegularFile, Seek};
use crate::inode::Inode;
use crate::result::Errno;

type Result<T> = core::result::Result<T, S3kErr>;


// fn load_shared_memory(socket: S3kCidx) -> Result<S3kCidx> {
//     let mem_cap = find_free_cap()?;
//     let request = loop {
//         let r = s3k_sock_recv(socket, mem_cap);
//         if r.err != S3kErr::Timeout {
//             break r;
//         }
//     };
//
//     s3k_pmp_load(mem_cap, BUFFER_PMP)?;
//     s3k_sync_mem();
//
//     info!("[server] Shared memory accepted at 0x{:x}", request.data[0]);
//     Ok(mem_cap)
// }

// fn unload_shared_memory(mem_cap: S3kCidx) -> Result<()> {
//
//     s3k_pmp_unload(mem_cap)?;
//     s3k_sync_mem();
//     s3k_cap_revoke(mem_cap)?;
//     Ok(())
// }

// Shared memory address (set after accepting shared memory)
struct FdTable {
    next_fd: i32,
    files: BTreeMap<i32, Arc<dyn File>>,
}

impl FdTable {
    fn new() -> Self {
        Self {
            next_fd: 3, // 0, 1, 2 reserved for stdin, stdout, stderr
            files: BTreeMap::new(),
        }
    }

    fn alloc(&mut self, file: Arc<dyn File>) -> i32 {
        let fd = self.next_fd;
        self.next_fd += 1;
        self.files.insert(fd, file);
        fd
    }

    fn get(&self, fd: i32) -> Option<Arc<dyn File>> {
        self.files.get(&fd).cloned()
    }

    fn close(&mut self, fd: i32) -> bool {
        self.files.remove(&fd).is_some()
    }
}

static FD_TABLE: Mutex<Option<FdTable>> = Mutex::new(None);
static FS: Mutex<Option<Arc<FAT32FileSystem>>> = Mutex::new(None);

fn init_fd_table() {
    *FD_TABLE.lock() = Some(FdTable::new());
}

fn with_fd_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut FdTable) -> R,
{
    let mut guard = FD_TABLE.lock();
    f(guard.as_mut().expect("FD table not initialized"))
}

fn get_fs() -> Arc<FAT32FileSystem> {
    FS.lock().as_ref().expect("Filesystem not initialized").clone()
}

/// Read a null-terminated string from shared memory
fn read_path_from_shared_mem(addr: u64, size: usize) -> alloc::string::String {
    let slice = unsafe { core::slice::from_raw_parts(addr as *const u8, size) };
    let end = slice.iter().position(|&b| b == 0).unwrap_or(size);
    alloc::string::String::from_utf8_lossy(&slice[..end]).into_owned()
}

/// Write data to shared memory
fn write_to_shared_mem(addr: u64, data: &[u8]) -> usize {
    let len = data.len();
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), addr as *mut u8, len);
    }
    len
}

/// Read data from shared memory
fn read_from_shared_mem(addr: u64, len: usize) -> Vec<u8> {
    let slice = unsafe { core::slice::from_raw_parts(addr as *const u8, len) };
    slice.to_vec()
}

/// Recursively lookup a path, returning the inode if found
/// Returns (parent_inode, final_component_name) if the parent exists but final component doesn't
fn lookup_path(root: Arc<dyn Inode>, path: &str) -> core::result::Result<Arc<dyn Inode>, (Arc<dyn Inode>, alloc::string::String, Errno)> {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Ok(root);
    }

    let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if components.is_empty() {
        return Ok(root);
    }

    let mut current = root;
    for (i, component) in components.iter().enumerate() {
        match current.clone().lookup_name(component) {
            Ok(child) => {
                current = child;
            }
            Err(e) => {
                return Err((current, alloc::string::String::from(*component), e));
            }
        }
    }

    Ok(current)
}

/// Recursively create directories for a path
fn mkdir_recursive(root: Arc<dyn Inode>, path: &str) -> core::result::Result<Arc<dyn Inode>, Errno> {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Ok(root);
    }

    let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if components.is_empty() {
        return Ok(root);
    }

    let mut current = root;
    for component in components.iter() {
        match current.clone().lookup_name(component) {
            Ok(child) => {
                if !child.metadata().ifmt.is_dir() {
                    return Err(Errno::ENOTDIR);
                }
                current = child;
            }
            Err(Errno::ENOENT) => {
                // Create the directory
                let new_dir = current.clone().create(InodeMode::def_dir(), component)?;
                current = new_dir;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(current)
}

/// Handle OPEN request
/// data[0] = REQ_OPEN
/// data[1] = flags
/// Path is read from shared memory
/// Returns: fd on success, negative errno on failure
fn handle_open(data: &[u64; 4]) -> i64 {
    let flags = OpenFlags::from_bits_truncate(data[1] as u32);
    let path_size = data[2] as usize;
    let path_ptr = data[3];
    let path = read_path_from_shared_mem(path_ptr, path_size);

    debug!("[server] open: path={}, flags={:?}", path, flags);

    let fs = get_fs();
    let root = fs.root();

    // Handle root directory
    if path == "/" || path.is_empty() {
        let file: Arc<dyn File> = DirFile::new(FileMeta::new(Some(root), flags));
        let fd = with_fd_table(|table| table.alloc(file));
        debug!("[server] open: allocated fd={}", fd);
        return fd as i64;
    }

    // Lookup the path recursively
    let last_component = path.split('/').filter(|s| !s.is_empty()).last();
    let inode = match lookup_path(root.clone(), &path) {
        Ok(inode) => inode,
        Err((parent, name, Errno::ENOENT)) if flags.contains(OpenFlags::O_CREAT) && name.eq(last_component.unwrap()) => {
            // Parent exists, create the file/directory
            let mode = if flags.contains(OpenFlags::O_DIRECTORY) {
                InodeMode::def_dir()
            } else {
                InodeMode::def_file()
            };

            match parent.create(mode, &name) {
                Ok(inode) => inode,
                Err(e) => return -(e as i64),
            }
        }
        Err((_, _, e)) => return -(e as i64),
    };

    // Check if opening directory without O_DIRECTORY when it's a dir
    let is_dir = inode.metadata().ifmt.is_dir();
    if flags.contains(OpenFlags::O_DIRECTORY) && !is_dir {
        return -(Errno::ENOTDIR as i64);
    }
    if !flags.contains(OpenFlags::O_DIRECTORY) && is_dir {
        return -(Errno::EISDIR as i64);
    }

    // Create file object
    let file: Arc<dyn File> = if is_dir {
        DirFile::new(FileMeta::new(Some(inode), flags))
    } else {
        RegularFile::new(FileMeta::new(Some(inode), flags))
    };

    // Handle O_TRUNC
    if flags.contains(OpenFlags::O_TRUNC) && !is_dir {
        if let Err(e) = file.truncate(0) {
            return -(e as i64);
        }
    }

    let fd = with_fd_table(|table| table.alloc(file));
    debug!("[server] open: allocated fd={}", fd);
    fd as i64
}

/// Handle CLOSE request
/// data[0] = REQ_CLOSE
/// data[1] = fd
/// Returns: 0 on success, negative errno on failure
fn handle_close(data: &[u64; 4]) -> i64 {
    let fd = data[1] as i32;
    debug!("[server] close: fd={}", fd);

    let success = with_fd_table(|table| table.close(fd));
    if success {
        0
    } else {
        -(Errno::EBADF as i64)
    }
}

/// Handle READ request
/// data[0] = REQ_READ
/// data[1] = fd
/// data[2] = count (bytes to read)
/// Returns: bytes read on success (data written to shared mem), negative errno on failure
fn handle_read(data: &[u64; 4]) -> i64 {
    let fd = data[1] as i32;
    let count = data[2] as usize;
    let buffer_addr = data[3];

    debug!("[server] read: fd={}, count={}", fd, count);

    let file = match with_fd_table(|table| table.get(fd)) {
        Some(f) => f,
        None => return -(Errno::EBADF as i64),
    };

    let mut buf = alloc::vec![0u8; count];
    match file.read(&mut buf) {
        Ok(n) => {
            write_to_shared_mem(buffer_addr,&buf[..n as usize]);
            n as i64
        }
        Err(e) => -(e as i64),
    }
}

/// Handle WRITE request
/// data[0] = REQ_WRITE
/// data[1] = fd
/// data[2] = count (bytes to write from shared mem)
/// Returns: bytes written on success, negative errno on failure
fn handle_write(data: &[u64; 4]) -> i64 {
    let fd = data[1] as i32;
    let count = data[2] as usize;
    let buffer_addr = data[3];

    debug!("[server] write: fd={}, count={}", fd, count);

    let file = match with_fd_table(|table| table.get(fd)) {
        Some(f) => f,
        None => return -(Errno::EBADF as i64),
    };

    let buf = read_from_shared_mem(buffer_addr, count);
    match file.write(&buf) {
        Ok(n) => n as i64,
        Err(e) => -(e as i64),
    }
}

/// Handle SEEK request
/// data[0] = REQ_SEEK
/// data[1] = fd
/// data[2] = offset
/// data[3] = whence (0=SET, 1=CUR, 2=END)
/// Returns: new offset on success, negative errno on failure
fn handle_seek(data: &[u64; 4]) -> i64 {
    let fd = data[1] as i32;
    let offset = data[2] as isize;
    let whence = data[3] as i32;

    debug!("[server] seek: fd={}, offset={}, whence={}", fd, offset, whence);

    let file = match with_fd_table(|table| table.get(fd)) {
        Some(f) => f,
        None => return -(Errno::EBADF as i64),
    };

    let seek = match Seek::try_from((whence, offset)) {
        Ok(s) => s,
        Err(e) => return -(e as i64),
    };

    match file.seek(seek) {
        Ok(pos) => pos as i64,
        Err(e) => -(e as i64),
    }
}

/// Handle LS (readdir) request
/// data[0] = REQ_LS
/// data[1] = fd
/// Returns: inode number on success (name written to shared mem), 0 if no more entries, negative errno on failure
fn handle_ls(data: &[u64; 4]) -> i64 {
    let fd = data[1] as i32;
    let buffer_addr = data[3];

    debug!("[server] ls: fd={}", fd);

    let file = match with_fd_table(|table| table.get(fd)) {
        Some(f) => f,
        None => return -(Errno::EBADF as i64),
    };

    match file.readdir() {
        Ok(Some((idx, inode))) => {
            let name = &inode.metadata().name;
            let name_bytes = if name.is_empty() {
                if idx == 0 { "." } else { ".." }
            } else {
                name.as_str()
            };
            write_to_shared_mem(buffer_addr, name_bytes.as_bytes());
            // Write null terminator
            unsafe {
                *((buffer_addr as usize + name_bytes.len()) as *mut u8) = 0;
            }
            inode.metadata().ino as i64
        }
        Ok(None) => 0, // No more entries
        Err(e) => -(e as i64),
    }
}

/// Handle SIZE request
/// data[0] = REQ_SIZE
/// data[1] = fd (if non-negative) OR path in shared memory (if fd < 0)
/// Returns: file size on success, negative errno on failure
fn handle_size(data: &[u64; 4]) -> i64 {
    let fd = data[1] as i64;

    debug!("[server] size: fd={}", fd);

    if fd >= 0 {
        // Get size from open file descriptor
        let file = match with_fd_table(|table| table.get(fd as i32)) {
            Some(f) => f,
            None => return -(Errno::EBADF as i64),
        };

        return match file.metadata().inode.as_ref() {
            Some(inode) => inode.metadata().inner.lock().size as i64,
            None => -(Errno::EBADF as i64),
        }
    }
    return -(Errno::EBADFD as i64);
}

/// Handle MKDIR request
/// data[0] = REQ_MKDIR
/// data[1] = recursive (1 = create parent dirs, 0 = fail if parent doesn't exist)
/// Path is read from shared memory
/// Returns: 0 on success, negative errno on failure
fn handle_mkdir(data: &[u64; 4]) -> i64 {
    let recursive = data[1] != 0;
    let path_size = data[2] as usize;
    let path_ptr = data[3];
    let path = read_path_from_shared_mem(path_ptr, path_size);

    debug!("[server] mkdir: path={}, recursive={}", path, recursive);

    let fs = get_fs();
    let root = fs.root();

    if path == "/" || path.is_empty() {
        return -(Errno::EEXIST as i64);
    }

    if recursive {
        // Create all missing directories in the path
        match mkdir_recursive(root, &path) {
            Ok(_) => 0,
            Err(e) => -(e as i64),
        }
    } else {
        // Only create the final directory, parent must exist
        match lookup_path(root.clone(), &path) {
            Ok(_) => {
                // Path already exists
                -(Errno::EEXIST as i64)
            }
            Err((parent, name, Errno::ENOENT)) => {
                // Parent exists, create the directory
                match parent.create(InodeMode::def_dir(), &name) {
                    Ok(_) => 0,
                    Err(e) => -(e as i64),
                }
            }
            Err((_, _, e)) => -(e as i64),
        }
    }
}

fn setup_other_app() -> Result<()> {
    let uart_addr = s3k_napot_encode(UART0_BASE_ADDR, 0x8);
    let app1_addr = s3k_napot_encode(APP_1_BASE_ADDR, APP_1_SIZE);

    info!("App1 pmp addr:{:#x}", app1_addr);
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

    // Keep a PMP for ourselves to rewrite APP1_memory
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

fn setup_socket() -> Result<S3kCidx> {
    let id_server = find_free_cap()?;
    let y_mode = S3kIpcMode::NOYIELD;
    let perm = S3kIpcPerm::SDATA | S3kIpcPerm::CDATA;
    s3k_cap_derive(CHANNEL, id_server, s3k_mk_socket(0, y_mode, perm, 0))?;
    let id_client = find_free_cap()?;
    s3k_cap_derive(id_server, id_client, s3k_mk_socket(0, y_mode, perm, 1))?;
    s3k_mon_cap_move(
        MONITOR,
        APP0_PID,
        id_client,
        APP1_PID,
        APP_1_CAP_SOCKET,
    )?;
    Ok(id_server)
}

fn run_other_app_with_schedule() -> Result<()>{
    s3k_reg_write(S3kReg::SERVTIME, 100);
    s3k_cap_delete(HART1_TIME)?;
    s3k_cap_delete(HART2_TIME)?;
    s3k_cap_delete(HART3_TIME)?;
    let cap = find_free_cap()?;
    s3k_cap_derive(HART0_TIME, cap, s3k_mk_time(0, 0, (S3K_SLOT_CNT / 2) as u16));
    s3k_mon_cap_move(MONITOR, APP0_PID, cap, APP1_PID, APP_1_TIME)?;
    //s3k_mon_cap_move(MONITOR, APP0_PID, HART1_TIME, APP1_PID, APP_1_TIME)?;
    s3k_sync();
    s3k_mon_resume(MONITOR, APP1_PID)?;
    //s3k_mon_yield(MONITOR, APP1_PID)?;
    Ok(())
}

fn handle_client(socket: S3kCidx) {
    loop {
        // Use a closure to ensure unload happens after processing
        let result = (|| {
            let request = loop {
                let r = s3k_sock_recv(socket, 0);
                if r.err != S3kErr::Timeout {
                    break r;
                }
            };

            if request.err != S3kErr::Success {
                warn!("[server] Receive error: {:?}", request.err);
                return None;
            }

            let req_code = request.data[0];
            debug!("[server] Received request code: {}", req_code);
            let result = match req_code {
                fs_common::REQ_OPEN => handle_open(&request.data),
                fs_common::REQ_CLOSE => handle_close(&request.data),
                fs_common::REQ_READ => handle_read(&request.data),
                fs_common::REQ_WRITE => handle_write(&request.data),
                fs_common::REQ_SEEK => handle_seek(&request.data),
                fs_common::REQ_LS => handle_ls(&request.data),
                fs_common::REQ_SIZE => handle_size(&request.data),
                fs_common::REQ_MKDIR => handle_mkdir(&request.data),
                _ => {
                    warn!("[server] Unknown request code: {}", req_code);
                    -(Errno::ENOSYS as i64)
                }
            };

            Some(result)
        })();

        // Send response if we have one
        if let Some(result) = result {
            let response = S3kMsg {
                data: [result as u64, 0, 0, 0],
                cap_idx: 0,
                send_cap: false,
            };

            let send_result = s3k_sock_send(socket, &response);
            if send_result != Ok(()) {
                warn!("[server] Send error: {:?}", send_result);
            }
        }
    }
}

fn _main() -> Result<()> {
    setup_uart_and_virtio()?;
    heap::init();
    fs_common::logger::init();
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

    // Store filesystem globally
    *FS.lock() = Some(fs.clone());

    // Initialize file descriptor table
    init_fd_table();

    let root = fs.root();
    let mut idx = 0;
    while let Ok(inode) = root.clone().lookup_idx(idx) {
        debug!("List file {}: {}", idx, inode.metadata().path);
        idx += 1;
    }

    // Setup app1 capabilities and PC
    setup_other_app()?;
    let socket = setup_socket()?;
    run_other_app_with_schedule()?;

    // Start handling client requests
    info!("[server] Starting request handler");
    handle_client(socket);

    Ok(())
}

#[unsafe(no_mangle)]
fn main() {
    if let Err(e) = _main() {
        panic!("fsd failed with: {:?}", e);
    }
}