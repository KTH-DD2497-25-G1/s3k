#![no_std]
#![no_main]
mod fs;

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use log::{debug, error, info};
use s3k_common::{heap, println};
use s3k_common::utils::APP_1_CAP_SOCKET;
use fs_common::OpenFlags;
use crate::fs::FileSystem;

type Result<T> = core::result::Result<T, ()>;

fn _main() -> Result<()> {
    heap::init();
    println!("Hello from App 1!");

    // The server moves the socket capability to APP_1_CAP_SOCKET (index 3)
    let fs = FileSystem::new(APP_1_CAP_SOCKET);

    let filename = "/hello.txt";
    let content = "Hello, S3K Filesystem!";

    // 1. Open file for writing (Create if not exists, Truncate if exists)
    println!("Opening {} for write...", filename);
    let fd = fs.open(filename, OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);

    if fd < 0 {
        println!("Failed to open file: error {}", fd);
        return Err(());
    }
    println!("File opened, fd: {}", fd);

    // 2. Write content
    println!("Writing data: '{}'", content);
    let bytes_written = fs.write(fd, content.as_bytes());
    if bytes_written < 0 {
        println!("Write failed: {}", bytes_written);
        fs.close(fd);
        return Err(());
    }
    println!("Wrote {} bytes", bytes_written);

    // 3. Seek back to start
    println!("Seeking to beginning...");
    let seek_res = fs.seek(fd, 0, 0); // 0 = Seek::Set
    if seek_res < 0 {
        println!("Seek failed: {}", seek_res);
        fs.close(fd);
        return Err(());
    }

    // 4. Read content back
    let mut buffer = vec![0u8; 100];
    println!("Reading data back...");
    let bytes_read = fs.read(fd, &mut buffer);
    if bytes_read < 0 {
        println!("Read failed: {}", bytes_read);
        fs.close(fd);
        return Err(());
    }

    let read_str = String::from_utf8_lossy(&buffer[..bytes_read as usize]);
    println!("Read {} bytes: '{}'", bytes_read, read_str);

    // 5. Verify
    if read_str == content {
        println!("SUCCESS: Read data matches written data!");
    } else {
        println!("FAILURE: Data mismatch!");
    }

    // 6. Close
    fs.close(fd);
    println!("File closed");
    Ok(())
}
#[unsafe(no_mangle)]
fn main() {
    if let Err(e) = _main() {
        panic!("fsd failed with: {:?}", e);
    }
}