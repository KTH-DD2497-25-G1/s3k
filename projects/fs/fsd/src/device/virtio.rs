use crate::device::BlockDevice;
use crate::result::{Errno, FsResult};
use alloc::collections::BTreeMap;
use core::ptr::NonNull;
use log::error;
use s3k_common::heap::{HeapFrameTracker, alloc_frames};
use spin::Mutex;
use virtio_drivers::device::blk::{SECTOR_SIZE, VirtIOBlk};
use virtio_drivers::transport::mmio::MmioTransport;
use virtio_drivers::{BufferDirection, Hal, PAGE_SIZE, PhysAddr};

const MMIO_SIZE: usize = 0x1000;

static VIRTIO_FRAMES: Mutex<BTreeMap<u64, HeapFrameTracker>> = Mutex::new(BTreeMap::new());

struct VirtioHal;

unsafe impl Hal for VirtioHal {
    fn dma_alloc(pages: usize, _: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let tracker = alloc_frames::<PAGE_SIZE>(PAGE_SIZE * pages);
        let ret = (tracker.ptr.addr().get() as u64, tracker.ptr);
        VIRTIO_FRAMES.lock().insert(ret.0, tracker);
        ret
    }

    unsafe fn dma_dealloc(paddr: PhysAddr, _: NonNull<u8>, _: usize) -> i32 {
        let mut frames = VIRTIO_FRAMES.lock();
        if frames.remove(&paddr).is_some() {
            0
        } else {
            -1
        }
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _: usize) -> NonNull<u8> {
        NonNull::new(paddr as *mut u8).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _: BufferDirection) -> PhysAddr {
        buffer.as_ptr().addr() as u64
    }

    unsafe fn unshare(_: PhysAddr, _: NonNull<[u8]>, _: BufferDirection) {}
}

type Blk = VirtIOBlk<VirtioHal, MmioTransport<'static>>;

pub struct VirtIOBlkDevice {
    base_addr: NonNull<u8>,
    block: Mutex<Blk>,
}

unsafe impl Send for VirtIOBlkDevice {}
unsafe impl Sync for VirtIOBlkDevice {}

impl BlockDevice for VirtIOBlkDevice {
    fn sector_size(&self) -> usize {
        SECTOR_SIZE
    }

    fn dev_size(&self) -> usize {
        SECTOR_SIZE * self.block.lock().capacity() as usize
    }

    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> FsResult {
        self.block
            .lock()
            .read_blocks(block_id, buf)
            .inspect_err(|e| error!("VirtIOBlock read error: {:?}", e))
            .map_err(|_| Errno::EIO)
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) -> FsResult {
        self.block
            .lock()
            .write_blocks(block_id, buf)
            .inspect_err(|e| error!("VirtIOBlock write error: {:?}", e))
            .map_err(|_| Errno::EIO)
    }
}

const VIRTIO0: usize = 0x10001000;

impl VirtIOBlkDevice {
    pub fn new() -> Self {
        let base_addr = NonNull::new(VIRTIO0 as *mut u8).unwrap();
        let transport = unsafe {
            MmioTransport::new(base_addr.cast(), MMIO_SIZE).unwrap()
        };
        let block = Blk::new(transport).unwrap();
        Self {
            base_addr: NonNull::new(VIRTIO0 as *mut u8).unwrap(),
            block: Mutex::new(block),
        }
    }
}
