use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use fs_common::OpenFlags;
use crate::ffi::{PAGE_SIZE};
use crate::inode::Inode;
use crate::result::{Errno, FsResult};

pub struct FileMeta {
    pub inode: Option<Arc<dyn Inode>>,
    pub flags: Mutex<OpenFlags>,
}

impl FileMeta {
    pub fn new(inode: Option<Arc<dyn Inode>>, flags: OpenFlags) -> Self {
        FileMeta {
            inode,
            flags: Mutex::new(flags),
        }
    }
}

/// https://man7.org/linux/man-pages/man2/lseek.2.html
pub enum Seek {
    /// The file offset is set to offset bytes.
    Set(isize),
    /// The file offset is set to its current location plus `offset` bytes.
    Cur(isize),
    /// The file offset is set to the size of the file plus `offset` bytes.
    End(isize),
}

impl TryFrom<(i32, isize)> for Seek {
    type Error = Errno;

    fn try_from((whence, offset): (i32, isize)) -> Result<Self, Self::Error> {
        match whence {
            0 => Ok(Seek::Set(offset)),
            1 => Ok(Seek::Cur(offset)),
            2 => Ok(Seek::End(offset)),
            _ => Err(Errno::EINVAL),
        }
    }
}

pub trait File: Send + Sync {
    fn metadata(&self) -> &FileMeta;

    fn read(&self, buf: &mut [u8]) -> FsResult<isize> {
        Err(Errno::EOPNOTSUPP)
    }

    fn write(&self, buf: &[u8]) -> FsResult<isize> {
        Err(Errno::EOPNOTSUPP)
    }

    fn truncate(&self, size: isize) -> FsResult {
        Err(Errno::EOPNOTSUPP)
    }

    fn seek(&self, seek: Seek) -> FsResult<isize> {
        Err(Errno::EOPNOTSUPP)
    }

    fn pread(&self, buf: &mut [u8], offset: isize) -> FsResult<isize> {
        Err(Errno::EOPNOTSUPP)
    }

    fn pwrite(&self, buf: &[u8], offset: isize) -> FsResult<isize> {
        Err(Errno::EOPNOTSUPP)
    }

    fn readdir(&self) -> FsResult<Option<(usize, Arc<dyn Inode>)>> {
        Err(Errno::ENOTDIR)
    }
}

impl dyn File {
    pub fn read_all(&self) -> FsResult<Vec<u8>> {
        self.seek(Seek::Set(0))?;
        let mut buf = Vec::new();
        let mut tmp = [0u8; PAGE_SIZE];
        loop {
            let len = self.read(&mut tmp)?;
            if len == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..len as usize]);
        }
        Ok(buf)
    }
}

pub struct DirFile {
    metadata: FileMeta,
    pos: Mutex<usize>,
}

impl DirFile {
    pub fn new(metadata: FileMeta) -> Arc<Self> {
        Arc::new(Self {
            metadata,
            pos: Mutex::default(),
        })
    }
}

impl File for DirFile {
    fn metadata(&self) -> &FileMeta {
        &self.metadata
    }

    fn read(&self, _buf: &mut [u8]) -> FsResult<isize> {
        Err(Errno::EISDIR)
    }

    fn readdir(&self) -> FsResult<Option<(usize, Arc<dyn Inode>)>> {
        let inode = self.metadata.inode.as_ref().unwrap();
        if inode.metadata().inner.lock().unlinked {
            return Err(Errno::ENOENT);
        }
        let mut pos = self.pos.lock();
        let inode = match *pos {
            0 => inode.clone(),
            1 => inode.metadata().parent.clone().and_then(|p| p.upgrade()).unwrap_or(inode.clone()),
            _ => match inode.clone().lookup_idx(*pos - 2) {
                Ok(inode) => inode,
                Err(Errno::ENOENT) => return Ok(None),
                Err(e) => return Err(e),
            },
        };
        *pos += 1;
        Ok(Some((*pos - 1, inode)))
    }
}

pub struct RegularFile {
    metadata: FileMeta,
    pos: Mutex<isize>,
    prw_lock: Mutex<()>,
}

impl RegularFile {
    pub fn new(metadata: FileMeta) -> Arc<Self> {
        Arc::new(Self {
            metadata,
            pos: Mutex::default(),
            prw_lock: Mutex::default(),
        })
    }
}

impl File for RegularFile {
    fn metadata(&self) -> &FileMeta {
        &self.metadata
    }

    fn read(&self, buf: &mut [u8]) -> FsResult<isize> {
        let inode = self.metadata.inode.as_ref().unwrap();
        let mut pos = self.pos.lock();
        let count = inode.read(buf, *pos)?;
        *pos += count;
        Ok(count)
    }

    fn write(&self, buf: &[u8]) -> FsResult<isize> {
        let inode = self.metadata.inode.as_ref().unwrap();
        let mut pos = self.pos.lock();
        let count = inode.write(buf, *pos)?;
        *pos += count;
        Ok(count)
    }

    fn truncate(&self, size: isize) -> FsResult {
        let inode = self.metadata.inode.as_ref().unwrap();
        inode.truncate(size)?;
        // The value of the seek pointer shall not be modified by a call to ftruncate().
        Ok(())
    }

    fn seek(&self, seek: Seek) -> FsResult<isize> {
        let mut pos = self.pos.lock();
        *pos = match seek {
            Seek::Set(offset) => {
                if offset < 0 {
                    return Err(Errno::EINVAL);
                }
                offset
            }
            Seek::Cur(offset) => {
                match pos.checked_add(offset) {
                    Some(new_pos) => new_pos,
                    None => return Err(if offset < 0 { Errno::EINVAL } else { Errno::EOVERFLOW }),
                }
            }
            Seek::End(offset) => {
                let size = self.metadata.inode.as_ref().unwrap().metadata().inner.lock().size;
                match size.checked_add(offset) {
                    Some(new_pos) => new_pos,
                    None => return Err(if offset < 0 { Errno::EINVAL } else { Errno::EOVERFLOW }),
                }
            }
        };
        Ok(*pos)
    }

    fn pread(&self, buf: &mut [u8], offset: isize) -> FsResult<isize> {
        let _lock = self.prw_lock.lock();
        let old = self.seek(Seek::Cur(0))?;
        self.seek(Seek::Set(offset))?;
        let ret = self.read(buf);
        self.seek(Seek::Set(old))?;
        ret
    }

    fn pwrite(&self, buf: &[u8], offset: isize) -> FsResult<isize> {
        let _lock = self.prw_lock.lock();
        let old = self.seek(Seek::Cur(0))?;
        self.seek(Seek::Set(offset))?;
        let ret = self.write(buf);
        self.seek(Seek::Set(old))?;
        ret
    }
}
