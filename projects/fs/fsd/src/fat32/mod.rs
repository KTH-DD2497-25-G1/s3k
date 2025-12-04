use crate::device::BlockDevice;
use crate::fat32::dir::FAT32Dirent;
use crate::fat32::fat::{FAT32Meta, FATEnt};
use crate::fat32::inode::FAT32Inode;
use crate::inode::Inode;
use crate::result::{Errno, FsResult};
use crate::utils::LateInit;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use bitvec_rs::BitVec;
use core::cmp::min;
use core::mem::ManuallyDrop;
use log::{info, trace, warn};

macro_rules! section {
    ($buf:ident, $start:ident, $end:ident) => {
        Self::split($buf, Self::$start, Self::$end)
            .try_into()
            .unwrap()
    };
}

mod bpb;
mod dir;
mod fat;
mod fsinfo;
mod inode;

const BLOCK_SIZE: usize = 512;
const BLOCK_CACHE_CAP: usize = 100;

const BOOT_SECTOR_ID: usize = 0;

pub struct FAT32FileSystem {
    device: Arc<dyn BlockDevice>,
    fat32meta: FAT32Meta,
    root: ManuallyDrop<LateInit<Arc<FAT32Inode>>>,
}

impl FAT32FileSystem {
    pub fn new(device: Arc<dyn BlockDevice>) -> FsResult<Arc<Self>> {
        let mut boot_sector = [0; BLOCK_SIZE];
        device.read_block(BOOT_SECTOR_ID, &mut boot_sector)?;
        let fs = Arc::new(FAT32FileSystem {
            device,
            fat32meta: FAT32Meta::new(&boot_sector)?,
            root: ManuallyDrop::new(LateInit::new()),
        });
        let root_cluster = fs.fat32meta.root_cluster as u32;
        fs.root.init(FAT32Inode::root(&fs, root_cluster)?);
        info!("FAT32 metadata: {:?}", fs.fat32meta);
        Ok(fs)
    }

    /// Read data based on cluster number and offset
    pub fn read_data(&self, cluster: usize, buf: &mut [u8], mut offset: usize) -> FsResult {
        buf.len()
            .checked_add(offset)
            .take_if(|v| *v <= self.fat32meta.bytes_per_cluster)
            .expect("Cross boundary");

        let mut cur = 0;
        let sector_start = self.fat32meta.data_sector_for_cluster(cluster);
        let sector_end = sector_start + self.fat32meta.sectors_per_cluster;
        for sector in sector_start..sector_end {
            if offset >= BLOCK_SIZE {
                offset -= BLOCK_SIZE;
                continue;
            }
            let next = min(cur + BLOCK_SIZE - offset, buf.len());
            self.device.read_block_offset(sector, &mut buf[cur..next], offset)?;
            offset = 0;
            cur = next;
            if cur == buf.len() {
                break;
            }
        }

        Ok(())
    }

    /// Write data based on cluster number and offset
    pub fn write_data(&self, cluster: usize, buf: &[u8], mut offset: usize) -> FsResult {
        buf.len()
            .checked_add(offset)
            .take_if(|v| *v <= self.fat32meta.bytes_per_cluster)
            .expect("Cross boundary");

        let mut cur = 0;
        let sector_start = self.fat32meta.data_sector_for_cluster(cluster);
        let sector_end = sector_start + self.fat32meta.sectors_per_cluster;
        for sector in sector_start..sector_end {
            if offset >= BLOCK_SIZE {
                offset -= BLOCK_SIZE;
                continue;
            }
            let next = min(cur + BLOCK_SIZE - offset, buf.len());
            self.device.write_block_offset(sector, &buf[cur..next], offset)?;
            offset = 0;
            cur = next;
            if cur == buf.len() {
                break;
            }
        }

        Ok(())
    }

    pub fn read_dir(
        self: &Arc<Self>,
        clusters: &[usize],
        occupy: &mut BitVec,
    ) -> FsResult<Vec<(FAT32Dirent, i32, i32)>> {
        let mut children = vec![];
        let mut dir = FAT32Dirent::default();
        let mut dir_pos = 0;
        let mut dir_len = 0;
        'outer: for cluster in clusters {
            let sector_start = self.fat32meta.data_sector_for_cluster(*cluster);
            let sector_end = sector_start + self.fat32meta.sectors_per_cluster;
            for sector in sector_start..sector_end {
                let mut buf = [0; BLOCK_SIZE];
                self.device.read_block_offset(sector, &mut buf, 0)?;
                for i in (0..BLOCK_SIZE).step_by(32) {
                    let value = &buf[i..i + 32];
                    if FAT32Dirent::is_end(value) {
                        break 'outer;
                    } else if FAT32Dirent::is_empty(value) {
                        occupy.push(false);
                        dir_pos += 1;
                        if dir_len > 0 {
                            warn!("Broken FAT32 dirent");
                            dir = FAT32Dirent::default();
                            dir_pos += dir_len;
                            dir_len = 0;
                        }
                    } else if FAT32Dirent::is_long_dirent(value) {
                        occupy.push(true);
                        dir_len += 1;
                        dir.append_long(value);
                    } else {
                        occupy.push(true);
                        dir_len += 1;
                        dir.append_short(value);
                        trace!(
                            "[fat32] Read fat32 dirent: {:<24} at {}-{} attr {:?}",
                            dir.name, dir_pos, dir_len, dir.attr
                        );
                        if dir.name != "." && dir.name != ".." {
                            children.push((dir, dir_pos, dir_len));
                        }
                        dir = FAT32Dirent::default();
                        dir_pos += dir_len;
                        dir_len = 0;
                    }
                }
            }
        }
        Ok(children)
    }

    pub fn write_dir(&self, clusters: &[usize], pos: usize, dirent: &[u8; 32]) -> FsResult<()> {
        let dirents_per_cluster = self.fat32meta.bytes_per_cluster / 32;
        let dirents_per_sector = BLOCK_SIZE / 32;
        let cluster = clusters[pos / dirents_per_cluster];
        let sector_start = self.fat32meta.data_sector_for_cluster(cluster);
        let sector_offset = (pos % dirents_per_cluster) / dirents_per_sector;
        let sector = sector_start + sector_offset;
        let block_offset = (pos % dirents_per_cluster) % dirents_per_sector * 32;
        self.device.write_block_offset(sector, dirent, block_offset)?;
        Ok(())
    }

    pub fn append_dir(
        &self,
        clusters: &mut Vec<usize>,
        occupy: &mut BitVec,
        dirent: &FAT32Dirent,
    ) -> FsResult<(usize, usize)> {
        let dirents_per_cluster = self.fat32meta.bytes_per_cluster / 32;
        let dirs = dirent.to_dirs();
        let mut left = 0;
        let mut right = 0;
        while right < occupy.len() {
            left = right;
            while left < occupy.len() && occupy[left] {
                left += 1;
                right = left;
            }
            while right < occupy.len() && right - left < dirs.len() && !occupy[right] {
                right += 1;
            }
            if right - left == dirs.len() {
                break;
            }
        }
        trace!(
            "Append FAT32 dirent: {:<8} at {}-{} attr {:?}",
            dirent.name,
            left,
            dirs.len(),
            dirent.attr
        );
        if right == occupy.len() {
            occupy.resize(left + dirs.len(), false);
            if occupy.len().div_ceil(dirents_per_cluster) > clusters.len() {
                let cluster = self.alloc_cluster()?;
                self.write_fat_ent(*clusters.last().unwrap(), FATEnt::NEXT(cluster as u32))?;
                clusters.push(cluster);
            }
            self.write_dir(clusters, occupy.len(), &FAT32Dirent::end())?;
        }
        for i in 0..dirs.len() {
            occupy.set(left + i, true);
            self.write_dir(clusters, left + i, &dirs[i])?;
        }
        Ok((left, dirs.len()))
    }

    pub fn remove_dir(
        &self,
        clusters: &mut Vec<usize>,
        occupy: &mut BitVec,
        pos: usize,
        len: usize,
    ) -> FsResult {
        let dirents_per_cluster = self.fat32meta.bytes_per_cluster / 32;
        if pos + len == occupy.len() {
            occupy.resize(pos, false);
            self.write_dir(clusters, pos, &FAT32Dirent::end())?;
            if occupy.len().div_ceil(dirents_per_cluster) < clusters.len() {
                let cluster = clusters.pop().unwrap();
                self.write_fat_ent(cluster, FATEnt::EMPTY)?;
            }
        } else {
            for i in 0..len {
                occupy.set(pos + i, false);
                self.write_dir(clusters, pos + i, &FAT32Dirent::empty())?;
            }
        }
        Ok(())
    }
}

impl FAT32FileSystem {
    fn root(&self) -> Arc<dyn Inode> {
        self.root.clone()
    }

    /// Calculate the block number and block offset of the FAT entry based on cluster number
    fn ent_block_for_cluster(&self, cluster: usize) -> (usize, usize) {
        let ent_sector = self.fat32meta.ent_sector_for_cluster(cluster);
        let ent_offset = self.fat32meta.ent_offset_for_cluster(cluster);
        let block_offset = ent_offset % BLOCK_SIZE;
        (ent_sector, block_offset)
    }

    /// Read FAT entry
    fn read_fat_ent(&self, cluster: usize) -> FsResult<FATEnt> {
        let (block_id, block_offset) = self.ent_block_for_cluster(cluster);
        let mut ent = [0; 4];
        self.device.read_block_offset(block_id, &mut ent, block_offset)?;
        Ok(FATEnt::from(u32::from_le_bytes(ent)))
    }

    /// Write FAT entry
    fn write_fat_ent(&self, cluster: usize, ent: FATEnt) -> FsResult {
        let (block_id, block_offset) = self.ent_block_for_cluster(cluster);
        self.device.write_block_offset(block_id, &u32::from(ent).to_le_bytes(), block_offset)?;
        Ok(())
    }

    fn walk_fat_ent(&self, mut ent: FATEnt) -> FsResult<Vec<usize>> {
        let mut clusters = vec![];
        while let FATEnt::NEXT(cluster) = ent {
            clusters.push(cluster as usize);
            ent = self.read_fat_ent(cluster as usize)?;
        }
        Ok(clusters)
    }

    /// Allocate a cluster
    fn alloc_cluster(&self) -> FsResult<usize> {
        let mut cluster = 0;
        for pos in 2..self.fat32meta.max_cluster {
            let fat_ent = self.read_fat_ent(pos)?;
            if fat_ent == FATEnt::EMPTY {
                cluster = pos;
                break;
            }
        }
        if cluster == 0 {
            warn!("Disk is full");
            return Err(Errno::ENOSPC);
        }
        self.write_fat_ent(cluster, FATEnt::EOF)?;
        let clear = vec![0; self.fat32meta.bytes_per_cluster];
        self.write_data(cluster, &clear, 0)?;
        Ok(cluster)
    }
}

impl Drop for FAT32FileSystem {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut self.root) };
        info!("FAT32FileSystem dropped");
    }
}
