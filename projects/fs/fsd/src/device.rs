use alloc::vec;
use crate::result::{Errno, FsResult};

pub trait BlockDevice: Send + Sync {
    /// Block size
    fn sector_size(&self) -> usize;

    /// Device size
    fn dev_size(&self) -> usize;

    /// Initialize after MMIO mapping is completed
    fn init(&self);

    /// Read data from block device
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> FsResult;

    /// Write data to block device
    fn write_block(&self, block_id: usize, buf: &[u8]) -> FsResult;

    fn read_block_offset(&self, block_id: usize, buf: &mut [u8], offset: usize) -> FsResult {
        if offset >= self.sector_size() {
            return Err(Errno::EINVAL);
        }
        let mut temp_buf = vec![0u8; self.sector_size()];
        self.read_block(block_id, &mut temp_buf)?;
        let len = buf.len().min(self.sector_size() - offset);
        buf[..len].copy_from_slice(&temp_buf[offset..offset + len]);
        Ok(())
    }

    fn write_block_offset(&self, block_id: usize, buf: &[u8], offset: usize) -> FsResult {
        if offset >= self.sector_size() {
            return Err(Errno::EINVAL);
        }
        let mut temp_buf = vec![0u8; self.sector_size()];
        self.read_block(block_id, &mut temp_buf)?;
        let len = buf.len().min(self.sector_size() - offset);
        temp_buf[offset..offset + len].copy_from_slice(&buf[..len]);
        self.write_block(block_id, &temp_buf)?;
        Ok(())
    }
}
