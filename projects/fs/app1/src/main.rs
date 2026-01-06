#![no_std]
#![no_main]
mod fs;

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;
use s3k_common::{heap, println, print};
use s3k_common::console::read_line;
use s3k_common::utils::APP_1_CAP_SOCKET;
use fs_common::OpenFlags;
use crate::fs::FileSystem;

type Result<T> = core::result::Result<T, ()>;

/// Helper to read a line from UART and return it as a trimmed String.
fn get_command(buffer: &mut [u8]) -> String {
    // Clear buffer to ensure no artifacts
    buffer.fill(0);

    // read_line populates the buffer and returns length
    let len = read_line(buffer);

    // Convert to string and trim whitespace/newlines
    let s = core::str::from_utf8(&buffer[..len]).unwrap_or("");
    String::from(s.trim())
}

/// Helper to parse mode strings (r, w, rw) into OpenFlags
fn parse_flags(mode: &str) -> OpenFlags {
    match mode {
        "r" => OpenFlags::O_RDONLY,
        "w" => OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC,
        "a" => OpenFlags::O_WRONLY | OpenFlags::O_APPEND | OpenFlags::O_CREAT,
        "rw" => OpenFlags::O_RDWR | OpenFlags::O_CREAT,
        _ => OpenFlags::O_RDONLY, // Default
    }
}

fn help() {
    println!("Available Commands:");
    println!("  open <path> <mode>     - Open file (mode: r, w, rw, a)");
    println!("  read <fd> <len>        - Read <len> bytes from <fd>");
    println!("  write <fd> <string>    - Write <string> to <fd>");
    println!("  seek <fd> <off> <loc>  - Seek (loc: 0=Set, 1=Cur, 2=End)");
    println!("  close <fd>             - Close <fd>");
    println!("  enc_on <pin>           - Enable Encryption");
    println!("  enc_off                - Disable Encryption");
    println!("  unlock <pin>           - Unlock Device");
}

fn _main() -> Result<()> {
    heap::init();
    println!("--- Filesystem Demo App ---");
    println!("Type 'help' for commands.");

    let fs = FileSystem::new(APP_1_CAP_SOCKET);
    let mut input_buf = [0u8; 128]; // Buffer for read_line

    loop {
        print!("> "); // Shell prompt

        let cmd_line = get_command(&mut input_buf);
        if cmd_line.is_empty() {
            continue;
        }

        // Split input into parts (command + args)
        let parts: Vec<&str> = cmd_line.split_whitespace().collect();
        let cmd = parts[0];
        match cmd {
            "help" => help(),

            "open" => {
                if parts.len() < 3 {
                    println!("Usage: open <path> <mode>");
                    continue;
                }
                let path = parts[1];
                let flags = parse_flags(parts[2]);

                let res = fs.open(path, flags);
                if res < 0 {
                    println!("Error opening '{}': {}", path, res);
                } else {
                    println!("Success. FD: {}", res);
                }
            },

            "read" => {
                if parts.len() < 3 {
                    println!("Usage: read <fd> <len>");
                    continue;
                }
                let fd = parts[1].parse::<i64>().unwrap_or(-1);
                let len = parts[2].parse::<usize>().unwrap_or(0);

                if len > 0 {
                    let mut buf = alloc::vec![0u8; len];
                    let res = fs.read(fd, &mut buf);
                    if res < 0 {
                        println!("Read Error: {}", res);
                    } else {
                        let read_str = String::from_utf8_lossy(&buf[..res as usize]);
                        println!("Read ({} bytes): \"{}\"", res, read_str);
                    }
                }
            },

            "write" => {
                if parts.len() < 3 {
                    println!("Usage: write <fd> <content>");
                    continue;
                }
                let fd = parts[1].parse::<i64>().unwrap_or(-1);
                // Reconstruct the message from the remaining parts
                // This allows writing strings with spaces like "Hello World"
                let content = parts[2..].join(" ");

                let res = fs.write(fd, content.as_bytes());
                if res < 0 {
                    println!("Write Error: {}", res);
                } else {
                    println!("Wrote {} bytes", res);
                }
            },

            "seek" => {
                if parts.len() < 4 {
                    println!("Usage: seek <fd> <offset> <whence>");
                    continue;
                }
                let fd = parts[1].parse::<i64>().unwrap_or(-1);
                let offset = parts[2].parse::<isize>().unwrap_or(0);
                let whence = parts[3].parse::<i32>().unwrap_or(0);

                let res = fs.seek(fd, offset, whence);
                if res < 0 {
                    println!("Seek Error: {}", res);
                } else {
                    println!("New offset: {}", res);
                }
            },

            "close" => {
                if parts.len() < 2 {
                    println!("Usage: close <fd>");
                    continue;
                }
                let fd = parts[1].parse::<i64>().unwrap_or(-1);
                let res = fs.close(fd);
                println!("Close result: {}", res);
            },

            "enc_on" => {
                if parts.len() < 2 {
                    println!("Usage: enc_on <pin>");
                    continue;
                }
                let pin = parts[1];
                let res = fs.enable_encryption(pin.len(), pin.as_bytes());
                println!("Enable Encryption result: {}", res);
            },

            "enc_off" => {
                let res = fs.disable_encryption();
                println!("Disable Encryption result: {}", res);
            },

            "unlock" => {
                if parts.len() < 2 {
                    println!("Usage: unlock <pin>");
                    continue;
                }
                let pin = parts[1];
                //print pin as bytes for debug
                println!("Unlocking with PIN bytes: {:?}", pin.as_bytes());
                let res = fs.unlock_device(pin.len(), pin.as_bytes());
                println!("Unlock result: {}", res);
            },

            _ => println!("Unknown command: '{}'. Type 'help' for list.", cmd),
        }
    }

    Ok(())
}

#[unsafe(no_mangle)]
fn main() {
    if let Err(e) = _main() {
        panic!("fsd shell failed with: {:?}", e);
    }
}