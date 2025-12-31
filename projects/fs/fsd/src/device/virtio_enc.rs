use crate::device::BlockDevice;
use crate::result::{Errno, FsResult};
use alloc::collections::BTreeMap;
use core::cmp::PartialEq;
use core::ptr::NonNull;
use log::{error, warn, debug, info};
use s3k_common::heap::{HeapFrameTracker, alloc_frames};
use spin::Mutex;
use virtio_drivers::device::blk::{SECTOR_SIZE, VirtIOBlk};
use virtio_drivers::transport::mmio::MmioTransport;
use virtio_drivers::{BufferDirection, Hal, PAGE_SIZE, PhysAddr};
use crate::device::tpm::tpm::UnlockError;
use crate::result::Errno::EUNDEF;

use aes::Aes256;
use aes::cipher::{KeyInit, BlockEncrypt, BlockDecrypt};
use xts_mode::{Xts128, get_tweak_default};
use crate::device::tpm::{TpmError, TpmDevice, init_tpm};

const MMIO_SIZE: usize = 0x1000;
const VIRTIO0: usize = 0x10001000;

const MAGIC_SIGNATURE: u32 = 0x67676767;
// Offset for actual data (Sector 0 is metadata)
const DATA_OFFSET_SECTORS: usize = 1;
static VIRTIO_FRAMES: Mutex<BTreeMap<u64, HeapFrameTracker>> = Mutex::new(BTreeMap::new());

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DeviceMetadata {
    pub magic: u32,
    pub version: u8,
    pub is_encrypted: u8, // 0 = Plain, 1 = Encrypted
    pub algo_id: u8,      // 0 = AES-256-XTS
    pub sealed_blob: [u8; 505], // Space for TPM sealed data, whole structure is 512 bytes equal to sector size
}

impl DeviceMetadata {
    pub fn new() -> Self {
        Self {
            magic: MAGIC_SIGNATURE,
            version: 1,
            is_encrypted: 0,
            algo_id: 0,
            sealed_blob: [0u8; 505],
        }
    }
    pub fn from_bytes(bytes: &[u8; SECTOR_SIZE]) -> Self {
        unsafe {
            // Cast the reference to a raw pointer
            let ptr = bytes.as_ptr() as *const DeviceMetadata;

            // "read_unaligned" handles the memory copy safely even if
            // the 'bytes' array isn't 4-byte aligned.
            core::ptr::read_unaligned(ptr)
        }
    }
    pub fn to_bytes(&self) -> [u8; SECTOR_SIZE] {
        unsafe {
            core::mem::transmute(*self)
        }
    }
}
#[derive(Debug)]
pub enum BlockOperationError {
    Undefined,
    UnSupportedAlgorithm,
    DeviceEncrypted,
    InvalidState,
    TpmError(TpmError),
    UnlockError(UnlockError),
    CryptoError,
    IOError,

}

impl Default for DeviceMetadata {
    fn default() -> Self {
        Self {
            magic: MAGIC_SIGNATURE,
            version: 0,
            is_encrypted: 0,
            algo_id: 0,
            sealed_blob: [0u8; 505],
        }
    }
}
const _: () = assert!(size_of::<DeviceMetadata>() == SECTOR_SIZE);
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

#[derive(Clone, PartialEq)]
pub enum WorkingMode {
    Normal = 0,
    Encrypted = 1,
    Unlocked = 2,
}

#[derive(Clone)]
pub struct EncryptionState {
    pub working_mode: WorkingMode,
    pub algo_id: u8,
    pub algo_param: [u8; 64],
    pub device_metadata: DeviceMetadata,
}
impl EncryptionState {
    pub fn new(working_mode: WorkingMode, algo_id: u8, algo_param: [u8; 64], device_metadata: DeviceMetadata) -> Self {
        Self {
            working_mode,
            algo_id,
            algo_param,
            device_metadata,
        }
    }
}
pub struct VirtIOBlkDeviceWithEncryption {
    base_addr: NonNull<u8>,
    block: Mutex<Blk>,
    state: Mutex<EncryptionState>,
}

unsafe impl Send for VirtIOBlkDeviceWithEncryption {}
unsafe impl Sync for VirtIOBlkDeviceWithEncryption {}

impl BlockDevice for VirtIOBlkDeviceWithEncryption {
    fn sector_size(&self) -> usize {
        SECTOR_SIZE
    }

    fn dev_size(&self) -> usize {
        SECTOR_SIZE * (self.block.lock().capacity() - DATA_OFFSET_SECTORS as u64) as usize
    }

    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> FsResult {
        let state = self.state.lock();
        match state.working_mode {
            WorkingMode::Normal => {
                self.block
                .lock()
                .read_blocks(block_id + DATA_OFFSET_SECTORS, buf)
                .inspect_err(|e| error!("VirtIOBlock read error: {:?}", e))
                .map_err(|_| Errno::EIO)
            }
            WorkingMode::Encrypted => {
                Err(EUNDEF)
            }
            WorkingMode::Unlocked => {
                self.block
                .lock()
                .read_blocks(block_id + DATA_OFFSET_SECTORS, buf)
                .inspect_err(|e| error!("VirtIOBlock read error: {:?}", e))
                .map_err(|_| Errno::EIO)?;
                match self.decrypt_data(buf, block_id, state.algo_id, &state.algo_param) {
                    Ok(_) => Ok(()),
                    Err(_) => Err(EUNDEF),
                }
            }
        }

    }

    fn write_block(&self, block_id: usize, buf: &[u8]) -> FsResult {
        use alloc::vec;
        let state = self.state.lock();
        match state.working_mode {
            WorkingMode::Normal => {
                return self.block
                .lock()
                .write_blocks(block_id + DATA_OFFSET_SECTORS, buf)
                .inspect_err(|e| error!("VirtIOBlock write error: {:?}", e))
                .map_err(|_| Errno::EIO);
            }
            WorkingMode::Encrypted => {
                return Err(EUNDEF);
            }
            WorkingMode::Unlocked => {
                let mut tmp_buf = vec![0u8; buf.len()];
                tmp_buf.copy_from_slice(buf);
                match self.encrypt_data(&mut tmp_buf, block_id, state.algo_id, &state.algo_param) {
                    Ok(_) => {
                        return self.block
                        .lock()
                        .write_blocks(block_id + DATA_OFFSET_SECTORS, &tmp_buf)
                        .inspect_err(|e| error!("VirtIOBlock write error: {:?}", e))
                        .map_err(|_| Errno::EIO);
                    }
                    Err(_) => {
                        return Err(EUNDEF);
                    }
                }
            }
        }
    }
}


impl VirtIOBlkDeviceWithEncryption {

    pub fn new() -> Self {
        let base_addr = NonNull::new(VIRTIO0 as *mut u8).unwrap();
        let transport = unsafe {
            MmioTransport::new(base_addr.cast(), MMIO_SIZE).unwrap()
        };
        let mut block = Blk::new(transport).unwrap();
        let mut meta_block = [0u8; SECTOR_SIZE];
        block.read_blocks(0, &mut meta_block).unwrap();
        let mut metadata = DeviceMetadata::from_bytes(&meta_block);
        if metadata.magic != MAGIC_SIGNATURE {
            warn!("Disk is uninitialized, formatting disk with destructive operation!");
            metadata = DeviceMetadata::new();
            let new_meta_bytes = metadata.to_bytes();
            block.write_blocks(0, &new_meta_bytes).unwrap();
            // Write 0 to all data sectors
            let zero_block = [0u8; SECTOR_SIZE];
            for i in DATA_OFFSET_SECTORS..block.capacity() as usize {
                block.write_blocks(i, &zero_block).unwrap();
            }
        }
        let working_mode = if metadata.is_encrypted == 0 {
            WorkingMode::Normal
        } else {
            WorkingMode::Encrypted
        };
        Self {
            base_addr: NonNull::new(VIRTIO0 as *mut u8).unwrap(),
            block: Mutex::new(block),
            state: Mutex::new(EncryptionState::new(working_mode, metadata.algo_id, [0u8; 64], metadata)),
        }
    }
    
    pub fn get_state(&self) -> EncryptionState {
        self.state.lock().clone()
    }
    pub fn enable_encryption(&self, algo_id: u8, pin: &[u8]) -> Result<(),BlockOperationError>{
        let mut state = self.state.lock();
        let mut block = self.block.lock();
        if state.working_mode != WorkingMode::Normal {
            return Err(BlockOperationError::InvalidState);
        }
        match algo_id {
            0 => {
                let tpm = init_tpm();
                tpm.get_random_number(&mut state.algo_param).map_err(|e| BlockOperationError::TpmError(e))?;
                let mut pub_blob = [0u8; 128];
                let mut priv_blob = [0u8; 256];
                let (pub_size, priv_size) =tpm.provision_master_key(pin, &mut state.algo_param, &mut pub_blob, &mut priv_blob).map_err(|e| BlockOperationError::TpmError(e))?;
                assert!(pub_size + priv_size <= state.device_metadata.sealed_blob.len());
                state.device_metadata.is_encrypted = 1;
                state.device_metadata.algo_id = algo_id;
                let mut idx = 0;
                //Write public size as u32
                state.device_metadata.sealed_blob[idx..idx+4].copy_from_slice(&(pub_size as u32).to_le_bytes());
                idx +=4;
                state.device_metadata.sealed_blob[idx..idx+pub_size].copy_from_slice(&pub_blob[..pub_size]);
                idx += pub_size;
                state.device_metadata.sealed_blob[idx..idx+4].copy_from_slice(&(priv_size as u32).to_le_bytes());
                idx +=4;
                state.device_metadata.sealed_blob[idx..idx+priv_size].copy_from_slice(&priv_blob[..priv_size]);
                //Write metadata back to disk
                let meta_bytes = state.device_metadata.to_bytes();
                block.write_blocks(0, &meta_bytes).map_err(|_| BlockOperationError::IOError)?;
                //Encrypt all data sectors
                info!("Enabling full disk encryption, please stand by");
                let mut buffer = [0u8; SECTOR_SIZE];
                for i in DATA_OFFSET_SECTORS..block.capacity() as usize {
                    block.read_blocks(i, &mut buffer).map_err(|_| BlockOperationError::IOError)?;
                    self.encrypt_data(&mut buffer, i - DATA_OFFSET_SECTORS, 0, &state.algo_param).map_err(|_| BlockOperationError::CryptoError)?;
                    block.write_blocks(i, &buffer).map_err(|_| BlockOperationError::IOError)?;
                }
                state.working_mode = WorkingMode::Unlocked;
                Ok(())
            }
            _ => {
                return Err(BlockOperationError::UnSupportedAlgorithm);
            }
        }

    }
    pub fn unlock_device(& self, pin: &[u8]) -> Result<(),BlockOperationError>{
        let mut state = self.state.lock();
        if state.working_mode != WorkingMode::Encrypted {
            return Err(BlockOperationError::InvalidState);
        }
        let tpm = init_tpm();
        let mut pub_size_bytes = [0u8;4];
        pub_size_bytes.copy_from_slice(&state.device_metadata.sealed_blob[0..4]);
        let pub_size = u32::from_le_bytes(pub_size_bytes) as usize;
        let mut priv_size_bytes = [0u8;4];
        priv_size_bytes.copy_from_slice(&state.device_metadata.sealed_blob[4+pub_size..8+pub_size]);
        let priv_size = u32::from_le_bytes(priv_size_bytes) as usize;
        let pub_blob = &state.device_metadata.sealed_blob[4..4+pub_size];
        let priv_blob = &state.device_metadata.sealed_blob[8+pub_size..8+pub_size+priv_size];
        let mut key_buf = [0u8;64];
        tpm.decrypt_master_key(pin, pub_blob, priv_blob, &mut key_buf).map_err(|e| BlockOperationError::UnlockError(e))?;
        state.algo_param.copy_from_slice(&key_buf);
        state.working_mode = WorkingMode::Unlocked;
        Ok(())
    }
    fn encrypt_data(&self, buffer: &mut [u8], block_id: usize, algo_id: u8, algo_param: &[u8]) -> Result<(),BlockOperationError>{
        match algo_id {
            0 => {
                let cipher_1 = Aes256::new(algo_param[0..32].into());
                let cipher_2 = Aes256::new(algo_param[32..64].into());

                // Initialize XTS mode.
                // Note: We use Xts128 because AES has a 128-bit BLOCK size.
                // The key size (256-bit) is determined by the `Aes256` generic.
                let xts = Xts128::<Aes256>::new(cipher_1, cipher_2);

                // Perform the encryption
                // `get_tweak_default` handles standard endianness for the sector index
                //catch panics from xts mode

                xts.encrypt_area(buffer, buffer.len(), block_id as u128, get_tweak_default);
                Ok(())
            }
            _ => {
                return Err(BlockOperationError::UnSupportedAlgorithm);
            }
        }
    }
    fn decrypt_data(&self, buffer: &mut [u8], block_id: usize, algo_id: u8, algo_param: &[u8]) -> Result<(),BlockOperationError>{
        match algo_id {
            0 => {
                let cipher_1 = Aes256::new(algo_param[0..32].into());
                let cipher_2 = Aes256::new(algo_param[32..64].into());

                // Initialize XTS mode.
                // Note: We use Xts128 because AES has a 128-bit BLOCK size.
                // The key size (256-bit) is determined by the `Aes256` generic.
                let xts = Xts128::<Aes256>::new(cipher_1, cipher_2);
                // Perform the decryption
                // `get_tweak_default` handles standard endianness for the sector index
                //catch panics from xts mode

                xts.decrypt_area(buffer, buffer.len(), block_id as u128, get_tweak_default);
                Ok(())
            }
            _ => {
                return Err(BlockOperationError::UnSupportedAlgorithm);
            }
        }
    }

}
