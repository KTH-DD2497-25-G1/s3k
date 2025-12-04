use crate::fat32::FAT32FileSystem;
use crate::fat32::dir::{FAT32Dirent, FileAttr};
use crate::fat32::fat::FATEnt;
use crate::ffi::InodeMode;
use crate::inode::{Inode, InodeInternal, InodeMeta};
use crate::result::{Errno, FsResult};
use crate::time::{TimeSpec, real_time};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use alloc::{format, vec};
use bitvec_rs::BitVec;
use core::cmp::min;
use core::sync::atomic::{AtomicUsize, Ordering};
use log::debug;
use spin::Mutex;

pub struct FAT32Inode {
    metadata: InodeMeta,
    fs: Weak<FAT32FileSystem>,
    dir_pos: usize,
    dir_len: usize,
    inner: Arc<Mutex<FAT32InodeInner>>,
}

struct FAT32InodeInner {
    dir_occupy: BitVec,
    clusters: Vec<usize>,
    children_loaded: bool,
    children: BTreeMap<String, Arc<FAT32Inode>>,
}

static INO_POOL: AtomicUsize = AtomicUsize::new(0);

impl FAT32Inode {
    pub fn root(fs: &Arc<FAT32FileSystem>, root_cluster: u32) -> FsResult<Arc<Self>> {
        let inode = Self {
            metadata: InodeMeta::new(
                INO_POOL.fetch_add(1, Ordering::Acquire),
                0,
                0,
                0,
                InodeMode::S_IFDIR,
                String::new(),
                String::new(),
                None,
                TimeSpec::default(),
                TimeSpec::default(),
                TimeSpec::default(),
                0,
            ),
            fs: Arc::downgrade(fs),
            dir_pos: 0,
            dir_len: 0,
            inner: Arc::new(Mutex::new(FAT32InodeInner {
                dir_occupy: BitVec::new(),
                clusters: fs.walk_fat_ent(FATEnt::NEXT(root_cluster))?,
                children_loaded: false,
                children: BTreeMap::new(),
            })),
        };
        Ok(Arc::new(inode))
    }

    fn load_children(self: Arc<Self>, inner: &mut FAT32InodeInner) -> FsResult {
        debug!("[fat32] Load children");
        let fs = self.fs.upgrade().ok_or(Errno::EIO)?;
        let children = fs.read_dir(&inner.clusters, &mut inner.dir_occupy)?;
        for child in children {
            let name = child.0.name;
            if name == "." || name == ".." {
                continue;
            }
            let inode = FAT32Inode {
                metadata: InodeMeta::new(
                    INO_POOL.fetch_add(1, Ordering::Acquire),
                    0,
                    0,
                    0,
                    if child.0.attr.contains(FileAttr::ATTR_DIRECTORY) {
                        InodeMode::S_IFDIR
                    } else {
                        InodeMode::S_IFREG
                    },
                    name.clone(),
                    format!("{}/{}", self.metadata().path, name),
                    Some(self.clone()),
                    TimeSpec::default(),
                    TimeSpec::default(),
                    TimeSpec::default(),
                    0,
                ),
                fs: Arc::downgrade(&fs),
                dir_pos: child.1 as usize,
                dir_len: child.2 as usize,
                inner: Arc::new(Mutex::new(FAT32InodeInner {
                    dir_occupy: BitVec::new(),
                    clusters: fs.walk_fat_ent(FATEnt::NEXT(child.0.cluster))?,
                    children_loaded: false,
                    children: BTreeMap::new(),
                })),
            };
            inner.children.insert(name, Arc::new(inode));
        }
        inner.children_loaded = true;
        Ok(())
    }
}

impl Inode for FAT32Inode {
    fn metadata(&self) -> &InodeMeta {
        &self.metadata
    }
}

impl InodeInternal for FAT32Inode {
    fn read_direct(&self, mut buf: &mut [u8], offset: isize) -> FsResult<isize> {
        let fs = self.fs.upgrade().ok_or(Errno::EIO)?;
        let file_size = self.metadata.inner.lock().size as usize;
        let mut offset = offset as usize;
        if offset >= file_size {
            return Ok(0);
        }
        let buf_end = min(buf.len(), file_size - offset);
        buf = &mut buf[..buf_end];

        let inner = self.inner.lock();
        let mut cur = 0;
        let cluster_start = offset / fs.fat32meta.bytes_per_cluster;
        offset %= fs.fat32meta.bytes_per_cluster;
        for cluster in &inner.clusters[cluster_start..] {
            let next = min(cur + fs.fat32meta.bytes_per_cluster - offset, buf.len());
            fs.read_data(*cluster, &mut buf[cur..next], offset)?;
            offset = 0;
            cur = next;
            if cur == buf.len() {
                break;
            }
        }

        Ok(buf.len() as isize)
    }

    fn write_direct(&self, buf: &[u8], offset: isize) -> FsResult<isize> {
        let fs = self.fs.upgrade().ok_or(Errno::EIO)?;
        let file_size = self.metadata.inner.lock().size as usize;
        let mut offset = offset as usize;
        if offset + buf.len() > file_size {
            self.truncate_direct((offset + buf.len()) as isize)?;
        }

        let inner = self.inner.lock();
        let mut cur = 0;
        let cluster_start = offset / fs.fat32meta.bytes_per_cluster;
        offset %= fs.fat32meta.bytes_per_cluster;
        for cluster in &inner.clusters[cluster_start..] {
            let next = min(cur + fs.fat32meta.bytes_per_cluster - offset, buf.len());
            fs.write_data(*cluster, &buf[cur..next], offset)?;
            offset = 0;
            cur = next;
            if cur == buf.len() {
                break;
            }
        }

        Ok(buf.len() as isize)
    }

    fn truncate_direct(&self, new_size: isize) -> FsResult {
        let fs = self.fs.upgrade().ok_or(Errno::EIO)?;
        let file_size = self.metadata.inner.lock().size as usize;
        let new_size = new_size as usize;
        if new_size == file_size {
            return Ok(());
        } else if new_size < file_size {
            let mut inner = self.inner.lock();
            let mut cluster_start = new_size.div_ceil(fs.fat32meta.bytes_per_cluster);
            if cluster_start == 0 {
                cluster_start = 1;
            }
            fs.write_fat_ent(inner.clusters[cluster_start - 1], FATEnt::EOF)?;
            if cluster_start < inner.clusters.len() {
                for cluster in &inner.clusters[cluster_start..] {
                    fs.write_fat_ent(*cluster, FATEnt::EMPTY)?;
                }
                inner.clusters.truncate(cluster_start);
            }
        } else {
            let mut inner = self.inner.lock();
            let mut cluster_start = file_size.div_ceil(fs.fat32meta.bytes_per_cluster);
            let cluster_end = new_size.div_ceil(fs.fat32meta.bytes_per_cluster);
            if cluster_start == 0 {
                cluster_start = 1;
            }
            if cluster_start < cluster_end {
                let mut prev = inner.clusters[cluster_start - 1];
                for _ in cluster_start..cluster_end {
                    let cluster = fs.alloc_cluster()?;
                    fs.write_fat_ent(prev, FATEnt::NEXT(cluster as u32))?;
                    prev = cluster;
                    inner.clusters.push(cluster);
                }
                fs.write_fat_ent(prev, FATEnt::EOF)?;
            }
        }
        self.metadata.inner.lock().size = new_size as isize;

        Ok(())
    }

    fn do_lookup_name(self: Arc<Self>, name: &str) -> FsResult<Arc<dyn Inode>> {
        let mut inner = self.inner.lock();
        if !inner.children_loaded {
            self.clone().load_children(&mut inner)?;
        }
        match inner.children.get(name) {
            Some(inode) => Ok(inode.clone()),
            None => Err(Errno::ENOENT),
        }
    }

    fn do_lookup_idx(self: Arc<Self>, idx: usize) -> FsResult<Arc<dyn Inode>> {
        let mut inner = self.inner.lock();
        if !inner.children_loaded {
            self.clone().load_children(&mut inner)?;
        }
        match inner.children.values().nth(idx) {
            Some(inode) => Ok(inode.clone()),
            None => Err(Errno::ENOENT),
        }
    }

    fn do_create(self: Arc<Self>, mode: InodeMode, name: &str) -> FsResult<Arc<dyn Inode>> {
        let fs = self.fs.upgrade().ok_or(Errno::EIO)?;
        let inner = &mut *self.inner.lock();
        let cluster = fs.alloc_cluster()?;
        let attr = if mode.is_dir() {
            FileAttr::ATTR_DIRECTORY
        } else {
            FileAttr::empty()
        };
        let dirent = FAT32Dirent::new(name.to_string(), attr, cluster as u32, 0);
        let (dir_pos, dir_len) =
            fs.append_dir(&mut inner.clusters, &mut inner.dir_occupy, &dirent)?;
        let now = real_time();
        let inode = Arc::new(Self {
            metadata: InodeMeta::new(
                INO_POOL.fetch_add(1, Ordering::Acquire),
                0,
                0,
                0,
                mode,
                name.to_string(),
                format!("{}/{}", self.metadata().path, name),
                Some(self.clone()),
                now.into(),
                now.into(),
                now.into(),
                0,
            ),
            fs: Arc::downgrade(&fs),
            dir_pos,
            dir_len,
            inner: Arc::new(Mutex::new(FAT32InodeInner {
                dir_occupy: BitVec::new(),
                clusters: vec![cluster],
                children_loaded: false,
                children: BTreeMap::new(),
            })),
        });
        if mode == InodeMode::S_IFDIR {
            let child_inner = &mut *inode.inner.lock();
            let parent_dir = FAT32Dirent::new(
                "..".to_string(),
                FileAttr::ATTR_DIRECTORY,
                inner.clusters[0] as u32,
                0,
            );
            let mut child_dir = dirent.clone();
            child_dir.name = ".".to_string();
            fs.append_dir(
                &mut child_inner.clusters,
                &mut child_inner.dir_occupy,
                &parent_dir,
            )?;
            fs.append_dir(
                &mut child_inner.clusters,
                &mut child_inner.dir_occupy,
                &child_dir,
            )?;
        }
        Ok(inode)
    }

    fn do_movein(self: Arc<Self>, name: &str, inode: Arc<dyn Inode>) -> FsResult {
        let fs = self.fs.upgrade().ok_or(Errno::EIO)?;
        let inner = &mut *self.inner.lock();
        match inode.downcast_arc::<FAT32Inode>() {
            Ok(inode) => {
                let attr = if inode.metadata().inner.lock().mode == InodeMode::S_IFDIR {
                    FileAttr::ATTR_DIRECTORY
                } else {
                    FileAttr::empty()
                };
                let cluster = inode.inner.lock().clusters[0];
                let dirent = FAT32Dirent::new(name.to_string(), attr, cluster as u32, 0);
                let (dir_pos, dir_len) =
                    fs.append_dir(&mut inner.clusters, &mut inner.dir_occupy, &dirent)?;
                let inode = Arc::new(Self {
                    metadata: InodeMeta::movein(
                        inode.as_ref(),
                        name.to_string(),
                        format!("{}/{}", self.metadata().path, name),
                        self.clone(),
                    ),
                    fs: Arc::downgrade(&fs),
                    dir_pos,
                    dir_len,
                    inner: inode.inner.clone(),
                });
                inner.children.insert(name.to_string(), inode);
                Ok(())
            }
            Err(_) => {
                todo!("Moving across file systems is not supported");
            }
        }
    }

    fn do_unlink(self: Arc<Self>, target: Arc<dyn Inode>) -> FsResult {
        let fs = self.fs.upgrade().ok_or(Errno::EIO)?;
        let inner = &mut *self.inner.lock();

        match target.downcast_arc::<FAT32Inode>() {
            Ok(target) => {
                fs.remove_dir(
                    &mut inner.clusters,
                    &mut inner.dir_occupy,
                    target.dir_pos,
                    target.dir_len,
                )?;
                inner.children.remove(&target.metadata().name);
            }
            Err(_) => {
                todo!("Unlinking across file systems is not supported");
            }
        }
        Ok(())
    }
}
