use crate::ffi::{Gid, InodeMode, OpenFlags, Uid};
use crate::file::{DirFile, File, FileMeta, RegularFile};
use crate::result::{Errno, FsResult};
use crate::time::TimeSpec;
use alloc::format;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use core::sync::atomic::{AtomicUsize, Ordering};
use downcast_rs::{DowncastSync, impl_downcast};
use spin::Mutex;

static KEY_COUNTER: AtomicUsize = AtomicUsize::new(1);

pub struct InodeMeta {
    pub key: usize,
    /// Inode number
    pub ino: usize,
    /// Inode device
    pub dev: u64,
    /// Inode type
    pub ifmt: InodeMode,
    /// File name
    pub name: String,
    /// File system path
    pub path: String,
    /// Parent directory
    pub parent: Option<Weak<dyn Inode>>,
    /// Mutable data
    pub inner: Arc<Mutex<InodeMetaInner>>,
}

pub struct InodeMetaInner {
    /// uid
    pub uid: Uid,
    /// gid
    pub gid: Gid,
    /// Access permissions
    pub mode: InodeMode,
    /// Number of hard links
    pub nlink: usize,
    /// Access time
    pub atime: TimeSpec,
    /// Modification time
    pub mtime: TimeSpec,
    /// Creation time
    pub ctime: TimeSpec,
    /// File size
    pub size: isize,
    /// Whether it has been deleted
    pub unlinked: bool,
}

impl InodeMeta {
    /// Create new Inode metadata
    ///
    /// If `parent` is `None`, it points to itself
    pub fn new(
        ino: usize,
        dev: u64,
        uid: Uid,
        gid: Gid,
        mode: InodeMode,
        name: String,
        path: String,
        parent: Option<Arc<dyn Inode>>,
        atime: TimeSpec,
        mtime: TimeSpec,
        ctime: TimeSpec,
        size: isize,
    ) -> Self {
        let inner = InodeMetaInner {
            uid,
            gid,
            mode,
            nlink: 1,
            atime,
            mtime,
            ctime,
            size,
            unlinked: false,
        };
        let key = KEY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let ifmt = mode & InodeMode::S_IFMT;
        let parent = parent.map(|parent| Arc::downgrade(&parent));
        Self {
            key,
            ino,
            dev,
            ifmt,
            name,
            path,
            parent,
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn new_simple(
        ino: usize,
        uid: Uid,
        gid: Gid,
        mode: InodeMode,
        name: String,
        parent: Arc<dyn Inode>,
    ) -> Self {
        let time = TimeSpec::default();
        let path = format!("{}/{}", parent.metadata().path, name);
        Self::new(
            ino,
            0,
            uid,
            gid,
            mode,
            name,
            path,
            Some(parent),
            time,
            time,
            time,
            0,
        )
    }

    pub fn movein(inode: &dyn Inode, name: String, path: String, parent: Arc<dyn Inode>) -> Self {
        Self {
            key: inode.metadata().key,
            ino: inode.metadata().ino,
            dev: inode.metadata().dev,
            ifmt: inode.metadata().ifmt,
            name,
            path,
            parent: Some(Arc::downgrade(&parent)),
            inner: inode.metadata().inner.clone(),
        }
    }
}

#[allow(unused)]
pub(super) trait InodeInternal {
    /// Read `buf` from `offset`, bypassing cache
    fn read_direct(&self, buf: &mut [u8], offset: isize) -> FsResult<isize> {
        Err(Errno::EPERM)
    }

    /// Write `buf` to `offset`, bypassing cache
    fn write_direct(&self, buf: &[u8], offset: isize) -> FsResult<isize> {
        Err(Errno::EPERM)
    }

    /// Set file size, bypassing cache
    fn truncate_direct(&self, size: isize) -> FsResult {
        Err(Errno::EPERM)
    }

    /// Look up directory entry by name
    fn do_lookup_name(self: Arc<Self>, name: &str) -> FsResult<Arc<dyn Inode>> {
        Err(Errno::EPERM)
    }

    /// Look up directory entry by index
    fn do_lookup_idx(self: Arc<Self>, idx: usize) -> FsResult<Arc<dyn Inode>> {
        Err(Errno::EPERM)
    }

    /// Create file/directory in current directory
    fn do_create(self: Arc<Self>, mode: InodeMode, name: &str) -> FsResult<Arc<dyn Inode>> {
        Err(Errno::EPERM)
    }

    /// Move file into current directory
    fn do_movein(self: Arc<Self>, name: &str, inode: Arc<dyn Inode>) -> FsResult {
        Err(Errno::EPERM)
    }

    /// Delete file from current directory
    fn do_unlink(self: Arc<Self>, target: Arc<dyn Inode>) -> FsResult {
        Err(Errno::EPERM)
    }
}

#[allow(private_bounds, unused)]
pub trait Inode: DowncastSync + InodeInternal {
    /// Get Inode metadata
    fn metadata(&self) -> &InodeMeta;
}
impl_downcast!(sync Inode);

impl dyn Inode {
    pub fn open(self: Arc<Self>, flags: OpenFlags) -> FsResult<Arc<dyn File>> {
        match self.metadata().ifmt {
            InodeMode::S_IFDIR => Ok(DirFile::new(FileMeta::new(Some(self), flags))),
            InodeMode::S_IFREG => Ok(RegularFile::new(FileMeta::new(Some(self), flags))),
            _ => Err(Errno::EPERM),
        }
    }

    pub fn read(&self, buf: &mut [u8], offset: isize) -> FsResult<isize> {
        self.read_direct(buf, offset)
    }

    pub fn write(&self, buf: &[u8], offset: isize) -> FsResult<isize> {
        self.write_direct(buf, offset)
    }

    pub fn truncate(&self, size: isize) -> FsResult {
        self.truncate_direct(size)
    }

    pub fn lookup_name(self: Arc<Self>, name: &str) -> FsResult<Arc<dyn Inode>> {
        if self.metadata().inner.lock().unlinked {
            return Err(Errno::ENOENT);
        }
        if !self.metadata().ifmt.is_dir() {
            return Err(Errno::ENOTDIR);
        }
        self.clone().do_lookup_name(name)
    }

    pub fn lookup_idx(self: Arc<Self>, idx: usize) -> FsResult<Arc<dyn Inode>> {
        if self.metadata().inner.lock().unlinked {
            return Err(Errno::ENOENT);
        }
        if !self.metadata().ifmt.is_dir() {
            return Err(Errno::ENOTDIR);
        }
        self.clone().do_lookup_idx(idx)
    }

    pub fn create(self: Arc<Self>, mode: InodeMode, name: &str) -> FsResult<Arc<dyn Inode>> {
        if self.metadata().inner.lock().unlinked {
            return Err(Errno::ENOENT);
        }
        if !self.metadata().ifmt.is_dir() {
            return Err(Errno::ENOTDIR);
        }
        self.do_create(mode, name)
    }

    pub fn movein(self: Arc<Self>, name: &str, inode: Arc<dyn Inode>) -> FsResult {
        if self.metadata().inner.lock().unlinked {
            return Err(Errno::ENOENT);
        }
        if !self.metadata().ifmt.is_dir() {
            return Err(Errno::ENOTDIR);
        }
        self.do_movein(name, inode)
    }

    pub fn unlink(self: Arc<Self>, name: &str) -> FsResult {
        if self.metadata().inner.lock().unlinked {
            return Err(Errno::ENOENT);
        }
        let inode = self.clone().lookup_name(name)?;
        inode.metadata().inner.lock().unlinked = true;
        self.do_unlink(inode)
    }
}
