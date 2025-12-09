#![no_std]
#![no_main]

use alloc::ffi::CString;
use alloc::string::String;
use s3k_common::ffi::{S3kCidx, S3kMsg};
use s3k_common::syscall::s3k_sock_sendrecv;
use fs_common::*;

pub struct FileSystem {
    socket: S3kCidx
}

impl FileSystem {
    pub fn new(sock: S3kCidx) -> Self {
        Self {
            socket: sock
        }
    }

    pub fn open(&self, path: &str, flag: OpenFlags) -> i64 {
        let cpath = CString::new(path).unwrap();
        let path_ptr = cpath.as_ptr() as u64;
        let path_len = cpath.to_bytes_with_nul().len() as u64;

        let request = S3kMsg {
            data: [REQ_OPEN, flag.bits() as u64, path_len, path_ptr],
            cap_idx: 0,
            send_cap: false
        };
        let reply = s3k_sock_sendrecv(self.socket, &request);
        reply.data[0] as i64
    }

    pub fn read(&self, fd: i64, buf: &mut [u8]) -> i64 {
        let request = S3kMsg {
            data: [REQ_READ, fd as u64, buf.len() as u64, buf.as_mut_ptr() as u64],
            cap_idx: 0,
            send_cap: false
        };
        let reply = s3k_sock_sendrecv(self.socket, &request);
        reply.data[0] as i64
    }

    pub fn write(&self, fd: i64, data: &[u8]) -> i64 {
        let request = S3kMsg {
            data: [REQ_WRITE, fd as u64, data.len() as u64, data.as_ptr() as u64],
            cap_idx: 0,
            send_cap: false
        };
        let reply = s3k_sock_sendrecv(self.socket, &request);
        reply.data[0] as i64
    }

    pub fn seek(&self, fd: i64, offset: isize, whence: i32) -> i64 {
        let request = S3kMsg {
            data: [REQ_SEEK, fd as u64, offset as u64, whence as u64],
            cap_idx: 0,
            send_cap: false
        };
        let reply = s3k_sock_sendrecv(self.socket, &request);
        reply.data[0] as i64
    }

    pub fn close(&self, fd: i64) -> i64 {
        let request = S3kMsg {
            data: [REQ_CLOSE, fd as u64, 0, 0],
            cap_idx: 0,
            send_cap: false
        };
        let reply = s3k_sock_sendrecv(self.socket, &request);
        reply.data[0] as i64
    }
}