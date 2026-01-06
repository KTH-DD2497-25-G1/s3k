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
use log::{debug, error, info, trace, warn};

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

        // Auto-format if metadata is empty/uninitialized.
        if Self::boot_sector_is_empty(&boot_sector) {
            warn!("[fat32] empty device detected; formatting device as FAT32");
            Self::format_device(&device)?;
            device.read_block(BOOT_SECTOR_ID, &mut boot_sector)?;
        }

        let fs = Arc::new(FAT32FileSystem {
            device,
            fat32meta: FAT32Meta::new(&boot_sector)?,
            root: ManuallyDrop::new(LateInit::new()),
        });

        let root_cluster = fs.fat32meta.root_cluster as u32;
        fs.root.init(FAT32Inode::root(&fs, root_cluster)?);
        debug!("[fat32] metadata: {:?}", fs.fat32meta);
        Ok(fs)
    }

    fn boot_sector_is_empty(bs: &[u8; BLOCK_SIZE]) -> bool {
        // Treat "all zeros" or missing boot signature as unformatted.
        bs.iter().all(|&b| b == 0) || bs[510] != 0x55 || bs[511] != 0xAA
    }

    /// Format the whole device as a fresh FAT32 volume.
    ///
    /// NOTE: This relies on the block device exposing total capacity in sectors.
    /// If your BlockDevice uses a different method name, adjust `num_blocks()`.
    /// Format the whole device as a fresh FAT32 volume.
    fn format_device(device: &Arc<dyn BlockDevice>) -> FsResult<()> {
        let total_sectors = (device.dev_size() / device.sector_size()) as u32;
        let bytes_per_sector = BLOCK_SIZE as u16;
        let reserved_sectors = 32u16;
        let num_fats = 2u8;
        let root_cluster = 2u32;
        let fsinfo_sector = 1u16;
        let backup_boot_sector = 6u16;

        // FAT32 requires count_of_clusters >= 65525. We iterate down from 8 (4KB) to 1 (512B).
        let mut sectors_per_cluster = 8u8;
        while sectors_per_cluster > 0 {
            let total_data_sectors = total_sectors.saturating_sub(reserved_sectors as u32);
            let approx_cluster_count = total_data_sectors / (sectors_per_cluster as u32);

            if approx_cluster_count >= 65525 {
                break;
            }
            sectors_per_cluster /= 2;
        }

        if sectors_per_cluster == 0 {
            error!("[fat32] Device too small for FAT32 (needs >= 65525 clusters)");
            return Err(Errno::ENOSPC);
        }

        debug!("[fat32] Formatting with SPC: {}, Total Sectors: {}", sectors_per_cluster, total_sectors);

        let mut fat_sz: u32 = 0;
        // Standard iterative approximation
        let mut current_fat_sz = (total_sectors * 4 + BLOCK_SIZE as u32 - 1) / BLOCK_SIZE as u32;
        for _ in 0..16 {
            fat_sz = current_fat_sz;
            let reserved_area = reserved_sectors as u32 + (num_fats as u32) * fat_sz;
            let data_sectors = match total_sectors.checked_sub(reserved_area) {
                Some(n) => n,
                None => return Err(Errno::ENOSPC),
            };

            let clusters = data_sectors / (sectors_per_cluster as u32);
            // FAT entries = clusters + 2 (reserved 0 and 1)
            let fat_bytes = (clusters + 2)
                .checked_mul(4)
                .ok_or(Errno::EOVERFLOW)?;

            current_fat_sz = (fat_bytes + (BLOCK_SIZE as u32 - 1)) / (BLOCK_SIZE as u32);
            if current_fat_sz == fat_sz {
                break;
            }
        }

        let fat_start = reserved_sectors as u32;
        let data_start = fat_start + (num_fats as u32) * fat_sz;

        // Build Boot Sector (BPB)
        let mut bs = [0u8; BLOCK_SIZE];
        bs[0] = 0xEB; bs[1] = 0x58; bs[2] = 0x90; // jmpBoot
        bs[3..11].copy_from_slice(b"MSWIN4.1");   // OEMName

        bs[11..13].copy_from_slice(&bytes_per_sector.to_le_bytes());
        bs[13] = sectors_per_cluster;
        bs[14..16].copy_from_slice(&reserved_sectors.to_le_bytes());
        bs[16] = num_fats;
        bs[17..19].copy_from_slice(&0u16.to_le_bytes()); // RootEntCnt (0 for FAT32)
        bs[19..21].copy_from_slice(&0u16.to_le_bytes()); // TotSec16 (0)
        bs[21] = 0xF8; // Media
        bs[22..24].copy_from_slice(&0u16.to_le_bytes()); // FATSz16 (0)
        bs[24..26].copy_from_slice(&63u16.to_le_bytes()); // SecPerTrk
        bs[26..28].copy_from_slice(&255u16.to_le_bytes()); // NumHeads
        bs[28..32].copy_from_slice(&0u32.to_le_bytes()); // HiddSec
        bs[32..36].copy_from_slice(&total_sectors.to_le_bytes()); // TotSec32

        bs[36..40].copy_from_slice(&fat_sz.to_le_bytes()); // FATSz32
        bs[40..42].copy_from_slice(&0u16.to_le_bytes()); // ExtFlags (Mirroring enabled, FAT 0 active)
        bs[42..44].copy_from_slice(&0u16.to_le_bytes()); // FSVer
        bs[44..48].copy_from_slice(&root_cluster.to_le_bytes());
        bs[48..50].copy_from_slice(&fsinfo_sector.to_le_bytes());
        bs[50..52].copy_from_slice(&backup_boot_sector.to_le_bytes());

        bs[64] = 0x80; // DrvNum
        bs[66] = 0x29; // BootSig
        bs[67..71].copy_from_slice(&0x1234_5678u32.to_le_bytes()); // VolID (random)
        bs[71..82].copy_from_slice(b"NO NAME    ");
        bs[82..90].copy_from_slice(b"FAT32   ");
        bs[510] = 0x55; bs[511] = 0xAA;

        // Build FSInfo Sector
        let mut fsinfo = [0u8; BLOCK_SIZE];
        fsinfo[0..4].copy_from_slice(&0x4161_5252u32.to_le_bytes()); // LeadSig (RRaA)
        fsinfo[484..488].copy_from_slice(&0x6141_7272u32.to_le_bytes()); // StrucSig (rrAa)
        // Free_Count: -1 (unknown) or calculated. -1 is safer for lazy init.
        fsinfo[488..492].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        // Nxt_Free: 2 (start looking at cluster 2)
        fsinfo[492..496].copy_from_slice(&2u32.to_le_bytes());
        fsinfo[510] = 0x55; fsinfo[511] = 0xAA;

        // Write Reserved Region
        device.write_block_offset(BOOT_SECTOR_ID, &bs, 0)?;
        device.write_block_offset(fsinfo_sector as usize, &fsinfo, 0)?;
        device.write_block_offset(backup_boot_sector as usize, &bs, 0)?;
        device.write_block_offset((backup_boot_sector + fsinfo_sector) as usize, &fsinfo, 0)?;

        // Zero out other reserved sectors (optional)
        let zero_block = [0u8; BLOCK_SIZE];
        // for s in 1..reserved_sectors {
        //     if s == fsinfo_sector || s == backup_boot_sector || s == (backup_boot_sector + fsinfo_sector) {
        //         continue;
        //     }
        //     device.write_block_offset(s as usize, &zero_block, 0)?;
        // }

        // Initialize FATs
        for fat_idx in 0..(num_fats as u32) {
            let base = fat_start + fat_idx * fat_sz;

            // FAT Entry 0: Media type (low byte) + Reserved
            // FAT Entry 1: Dirty bit / End of Chain marker
            // FAT Entry 2: EOF (Root Directory End)
            let mut first_fat_sector = [0u8; BLOCK_SIZE];
            let e0: u32 = 0x0FFF_FFF8 | (0xF8u32);
            let e1: u32 = 0x0FFF_FFFF;
            let e2: u32 = 0x0FFF_FFFF;

            first_fat_sector[0..4].copy_from_slice(&e0.to_le_bytes());
            first_fat_sector[4..8].copy_from_slice(&e1.to_le_bytes());
            first_fat_sector[8..12].copy_from_slice(&e2.to_le_bytes());

            device.write_block_offset(base as usize, &first_fat_sector, 0)?;

            // Zero the rest of the FAT.
            for i in 1..fat_sz {
                device.write_block_offset((base + i) as usize, &zero_block, 0)?;
            }
        }

        // Initialize Root Directory
        let root_sector = data_start + (root_cluster - 2) * (sectors_per_cluster as u32);
        for i in 0..(sectors_per_cluster as u32) {
            device.write_block_offset((root_sector + i) as usize, &zero_block, 0)?;
        }

        Ok(())
    }

    pub fn root(&self) -> Arc<dyn Inode> {
        self.root.clone()
    }

    /// Read data based on cluster number and offset
    fn read_data(&self, cluster: usize, buf: &mut [u8], mut offset: usize) -> FsResult {
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
    fn write_data(&self, cluster: usize, buf: &[u8], mut offset: usize) -> FsResult {
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

    fn read_dir(
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
                            warn!("[fat32] Broken FAT32 dirent");
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

    fn write_dir(&self, clusters: &[usize], pos: usize, dirent: &[u8; 32]) -> FsResult<()> {
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

    fn update_dir(&self, clusters: &[usize], pos: usize, size: usize) -> FsResult<()> {
        let dirents_per_cluster = self.fat32meta.bytes_per_cluster / 32;
        let dirents_per_sector = BLOCK_SIZE / 32;
        let cluster = clusters[pos / dirents_per_cluster];
        let sector_start = self.fat32meta.data_sector_for_cluster(cluster);
        let sector_offset = (pos % dirents_per_cluster) / dirents_per_sector;
        let sector = sector_start + sector_offset;
        let block_offset = (pos % dirents_per_cluster) % dirents_per_sector * 32;
        let mut buf = [0; 32];
        self.device.read_block_offset(sector, &mut buf, block_offset)?;
        buf[28..32].copy_from_slice(&(size as u32).to_le_bytes());
        self.device.write_block_offset(sector, &buf, block_offset)?;
        Ok(())
    }

    fn append_dir(
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

    fn remove_dir(
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
            warn!("[fat32] Disk is full");
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
        info!("[fat32] FAT32FileSystem dropped");
    }
}
