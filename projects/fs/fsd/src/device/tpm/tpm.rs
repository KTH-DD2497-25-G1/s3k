pub const TPM_TIS_BASE: usize = 0x0400_0000;
pub const TPM_TIS_SIZE: usize = 0x0000_5000;

// TPM TIS register offsets (from base)
const TPM_ACCESS:    usize = 0x0000;
const TPM_STS:       usize = 0x0018;
const TPM_DATA_FIFO: usize = 0x0024;

const TPM_DID_VID:   usize = 0x0F00;
const TPM_RID:       usize = 0x0F04;


const TPM_ACCESS_VALID:            u8 = 1 << 7; 
const TPM_ACCESS_ACTIVE_LOCALITY:  u8 = 1 << 5; 
const TPM_ACCESS_REQUEST_USE:      u8 = 1 << 1; 


const TPM_STS_VALID:            u8 = 1 << 7;
const TPM_STS_COMMAND_READY:    u8 = 1 << 6;
const TPM_STS_TPM_GO:           u8 = 1 << 5;
const TPM_STS_DATA_AVAIL:       u8 = 1 << 4;
const TPM_STS_EXPECT:           u8 = 1 << 3;

const TPM2_ST_NO_SESSIONS: u16 = 0x8001;
const TPM2_ST_SESSIONS: u16 = 0x8002;
const TPM_RS_PW: u32 = 0x40000009;

const TPM2_CC_GET_RANDOM:  u32 = 0x0000017B;

const TPM2_CC_STARTUP: u32 = 0x00000144;
const TPM2_SU_CLEAR: u16 = 0x0000;

const TPM2_CC_CREATE_PRIMARY: u32 = 0x00000131;
const TPM2_CC_CREATE:         u32 = 0x00000153;
const TPM2_CC_FLUSH_CONTEXT:  u32 = 0x00000165;

const TPM_SRK_ATTRS_FIXEDTPM:          u32 = 1 << 1;
const TPM_SRK_ATTRS_FIXEDPARENT:       u32 = 1 << 4;
const TPM_SRK_ATTRS_SENSITIVEDATAORIGIN:u32 = 1 << 5;
const TPM_SRK_ATTRS_USERWITHAUTH:      u32 = 1 << 6;
const TPM_SRK_ATTRS_NODA:              u32 = 1 << 10;
const TPM_SRK_ATTRS_RESTRICTED:        u32 = 1 << 16;
const TPM_SRK_ATTRS_DECRYPT:          u32 = 1 << 17;

const TPM_RH_OWNER:           u32 = 0x40000001;
const TPM_RH_NULL:            u32 = 0x40000007;
const TPM_ALG_RSA:     u16 = 0x0001;
const TPM_ALG_AES:     u16 = 0x0006;
const TPM_ALG_SHA256:  u16 = 0x000B;
const TPM_ALG_NULL:    u16 = 0x0010;
const TPM_ALG_AES_CFB: u16 = 0x0043;
const TPM_ALG_ECC:         u16 = 0x0023;
const TPM_ECC_NIST_P256:   u16 = 0x0003;
const TPM_ALG_KDF2_HMAC_SHA256: u16 = 0x0022;

const TPM_ALG_KEYEDHASH: u16 = 0x0008;

const TPM2_CC_LOAD:   u32 = 0x00000157;
const TPM2_CC_UNSEAL: u32 = 0x0000015E;

const TPM_RC_LOCKOUT:   u16 = 0x0921;
const TPM_RC_AUTH_FAIL: u16 = 0x008E;


pub struct TpmDevice(usize);

impl TpmDevice {
    pub fn new(base_addr: usize) -> Self {
        TpmDevice(base_addr)
    }
}

#[derive(Debug)]
pub enum TpmError{
    Timeout,
    BufferTooSmall,
    NotReady,
    ResponseError(u16)

}
#[derive(Debug)]
pub enum UnlockError {
    TpmError(TpmError),
    WrongPin,
    Lockout,
}
trait AsPtr {
    fn as_ptr_u8(&self) -> *mut u8;
    fn as_ptr_u32(&self) -> *mut u32;
}

impl AsPtr for usize {
    fn as_ptr_u8(&self) -> *mut u8 {
        *self as *mut u8
    }

    fn as_ptr_u32(&self) -> *mut u32 {
        *self as *mut u32
    }
}

impl TpmDevice {

    fn send_raw_command(&self, command: &[u8], response: &mut [u8]) -> Result<usize, TpmError>{
        use log::debug;
    
        debug!("[TPM] Starting send_raw_command");

        if !self.command_ready() {
            debug!("[TPM] Failed to set command ready");
        return Err(TpmError::NotReady);
    }

        self.write_command_to_fifo(command)?;
        debug!("[TPM] Command written to FIFO");

        self.start_execution();
        debug!("[TPM] Execution started, TPM_GO bit set");

        let num = self.read_response_from_fifo(response)?;
        debug!("[TPM] Response read successfully");

        Ok(num)
    }

    fn start_execution(&self) {
            self.write_sts(TPM_STS_TPM_GO);
        }

    fn write_command_to_fifo(&self, command: &[u8]) -> Result<(), TpmError> {
        use log::debug;
        let mut written = 0;

        let sts = self.read_sts();
        let ready = (sts & TPM_STS_COMMAND_READY as u32) != 0;
        if !ready {
            debug!("[TPM] Not ready, STS: 0x{:x}", sts);
            return Err(TpmError::NotReady);
        }

        debug!("[TPM] STS ready, writing {} bytes to FIFO", command.len());

        while written < command.len() {
            let status = self.read_sts();

            let mut burst = (status >> 8) & 0xFFFF;
            if burst == 0 {
                //wait for a small delay
                for _ in 0..1000 {
                    unsafe {
                        core::arch::asm!("nop");
                    }
                }
                continue;
            }

            // 3. Write min(remaining, burst)
            let remaining = command.len() - written;
            let to_write = core::cmp::min(remaining, burst as usize);

            for _ in 0..to_write {
                self.fifo_write_byte(command[written]);
                written += 1;
            }

        }

        // 5. Trigger Execution
        self.write_sts(TPM_STS_TPM_GO);
        debug!("[TPM] All bytes written to FIFO");
    
        Ok(())
    }

    fn read_response_from_fifo(&self, response: &mut [u8]) -> Result<usize, TpmError>{
        let mut ok = false;
        for _ in 0..1_000_000{
            let status = self.read_sts();
            if (status & TPM_STS_DATA_AVAIL as u32) !=0 {
                ok = true;
                break;
            }
        }
        
        if !ok {
            return Err(TpmError::Timeout);
        }

        let mut index = 0;
        loop{
            if index >= response.len(){
                return Err(TpmError::BufferTooSmall);
            }

            //REad one byte from the FIFO
            let b = self.fifo_read_byte();
            response[index] = b;
            index += 1;

            // Check if there is more data available
            let status = self.read_sts();
            if (status & TPM_STS_DATA_AVAIL as u32) == 0 {
                break;
            }
        }
        Ok(index)
    }

    fn access_ptr(&self) -> *mut u8 {
        (self.0 + TPM_ACCESS).as_ptr_u8()
    }

    fn sts_ptr(&self) -> *mut u32 {
        (self.0 + TPM_STS).as_ptr_u32()
    }

    fn fifo_ptr(&self) -> *mut u8 {
        (self.0 + TPM_DATA_FIFO).as_ptr_u8()
    }

    fn did_vid_ptr(&self) -> *mut u32 {
        (self.0 + TPM_DID_VID).as_ptr_u32()
    }

    fn rid_ptr(&self) -> *mut u8 {
        (self.0 + TPM_RID).as_ptr_u8()
    }

    fn read_did_vid(&self) -> (u16, u16){
        unsafe {
            let did_vid = self.did_vid_ptr().read_volatile();
            let did = (did_vid >> 16) as u16;
            let vid = (did_vid & 0xFFFF) as u16;
            (did, vid)
        }
    }

    pub fn read_rid(&self) -> u8 {
        unsafe { self.rid_ptr().read_volatile() }
    }

    fn read_access(&self) -> u8{
        unsafe { self.access_ptr().read_volatile()}
    }

    fn write_access(&self, value: u8){
        unsafe { self.access_ptr().write_volatile(value)}
    }

    fn read_sts(&self) -> u32{
        //STS is 32 bits wide, so read 4 bytes
        unsafe { self.sts_ptr().read_volatile() as u32}

    }

    fn write_sts(&self, value: u8){
        unsafe { self.sts_ptr().write_volatile(value as u32)}
    }

    pub fn request_locality_0(&self) -> bool{
        self.write_access(TPM_ACCESS_REQUEST_USE);

        //poll until valid + active locailty bits are set
        for _ in 0..1_000_000{ //arbitrary number just dont want it to freeze with a while loop
            let access = self.read_access();
            let valid = (access & TPM_ACCESS_VALID) != 0;
            let active = (access & TPM_ACCESS_ACTIVE_LOCALITY) != 0;

            if valid && active {
                return true
            }
        }

        return false
    }

    pub fn command_ready(&self) -> bool {
        use log::debug;

        self.write_sts(TPM_STS_COMMAND_READY);

        for i in 0..1_000_000 { //arbitrary number just dont want it to freeze with a while loop
            let sts = self.read_sts();
            let ready = (sts & TPM_STS_COMMAND_READY as u32) != 0;

            if ready {
                debug!("[TPM] Command ready after {} iterations, STS: 0x{:02x}", i, sts);
                return true;
            }
        }

        false
    }

    fn fifo_write_byte(&self, input_byte:u8) {
        unsafe {
            self.fifo_ptr().write_volatile(input_byte)
        }
    }

    fn fifo_read_byte(&self) -> u8 {
        unsafe {
            self.fifo_ptr().read_volatile()
        }
    }


    pub fn get_random_number(&self, output: &mut [u8]) -> Result<usize, TpmError>{
        use log::debug;

        
        let requested = output.len() as u16;
        let mut command_buffer = [0u8;12];
        let commlen = command_buffer.len();

        //tag
        put_u16_be(&mut command_buffer, 0, TPM2_ST_NO_SESSIONS);
        //length
        put_u32_be(&mut command_buffer, 2, commlen as u32);
        //command
        put_u32_be(&mut command_buffer, 6, TPM2_CC_GET_RANDOM);
        //bytes
        put_u16_be(&mut command_buffer, 10, requested);
        
        debug!("[TPM] Command buffer: {:02x?}", &command_buffer);
        debug!("[TPM] Sending GetRandom command, requesting {} bytes", requested);

        //response buffer big enough for header + data
        let mut response = [0u8;512];

        //sending command, response contains the response (obviously) 
        let response_length = self.send_raw_command(&command_buffer, &mut response)?;

        debug!("[TPM] Got response of {} bytes", response_length);
        debug!("[TPM] Response bytes: {:02x?}", &response[..response_length]);

        if response_length < 10{
            return Err(TpmError::BufferTooSmall);
        }

        let _tag = get_u16_be(&response, 0);
        let _size = get_u32_be(&response, 2);
        let response_code = get_u32_be(&response, 6);

        if response_code != 0{
            debug!("[TPM] ERROR: Non-zero response code: 0x{:08x}", response_code);
            return Err(TpmError::ResponseError((response_code & 0xFFFF) as u16))    
        };

        let rand_size = get_u16_be(&response, 10) as usize;

        let n = core::cmp::min(rand_size, output.len());

        output[..n].copy_from_slice(&response[12 .. 12 + n]);

        Ok(n)

    }

    /// Generates a 32-byte AES key and seals it against a PIN.
    /// Returns: (Public Blob, Private Blob) - Store these on your disk header!
    pub fn provision_master_key(&self, pin: &[u8], master_key: &[u8], output_pub: &mut [u8], output_priv: &mut [u8]) -> Result<(usize, usize), TpmError> {
        use log::{debug};
        // We create a root key in the Owner hierarchy to act as the parent, this key is not exportable.
        let srk_handle = self.create_primary_srk()?;
        debug!("[FDE] SRK Created. Handle: 0x{:08x}", srk_handle);

        // This encrypts 'master_key' using 'srk_handle' and binds it to 'pin'
        let (pub_size, priv_size) = self.tpm2_create(srk_handle, pin, &master_key, output_pub, output_priv)?;
        debug!("[FDE] Master Key Sealed successfully.");

        self.flush_context(srk_handle)?;

        Ok((pub_size, priv_size))
    }

    fn create_primary_srk(&self) -> Result<u32, TpmError> {
        let mut cmd = [0u8; 256];
        let mut idx = 0;

        // --- Header ---
        put_u16_be(&mut cmd, idx, TPM2_ST_SESSIONS); idx += 2;
        idx += 4; // Skip size
        put_u32_be(&mut cmd, idx, TPM2_CC_CREATE_PRIMARY); idx += 4;

        // Handle: TPM_RH_OWNER
        put_u32_be(&mut cmd, idx, TPM_RH_OWNER); idx += 4;
        // Authorization Area (Mandatory for TPM_RH_OWNER)
        put_u32_be(&mut cmd, idx, 9); idx += 4;
        put_u32_be(&mut cmd, idx, TPM_RS_PW); idx += 4;
        put_u16_be(&mut cmd, idx, 0); idx += 2;
        put_u8(&mut cmd, idx, 0x00); idx += 1;
        put_u16_be(&mut cmd, idx, 0); idx += 2;
        // Sensitive (Empty for SRK)
        put_u16_be(&mut cmd, idx, 4); idx += 2;
        put_u16_be(&mut cmd, idx, 0); idx += 2;
        put_u16_be(&mut cmd, idx, 0); idx += 2;

        let pub_start = idx;
        idx += 2; // Skip public size

        // 1. Type: RSA
        put_u16_be(&mut cmd, idx, TPM_ALG_RSA); idx += 2;

        // 2. Name Alg: SHA256
        put_u16_be(&mut cmd, idx, TPM_ALG_SHA256); idx += 2;

        // 3. Attributes
        let srk_attrs: u32 =
            TPM_SRK_ATTRS_FIXEDTPM
                | TPM_SRK_ATTRS_FIXEDPARENT
                | TPM_SRK_ATTRS_SENSITIVEDATAORIGIN
                | TPM_SRK_ATTRS_USERWITHAUTH
                | TPM_SRK_ATTRS_NODA
                | TPM_SRK_ATTRS_RESTRICTED
                | TPM_SRK_ATTRS_DECRYPT;
        put_u32_be(&mut cmd, idx, srk_attrs); idx += 4;

        // 4. Auth Policy (Size 0)
        put_u16_be(&mut cmd, idx, 0); idx += 2;

        // 5. RSA Parameters (TPMS_RSA_PARMS)
        // A. Symmetric (TPMT_SYM_DEF_OBJECT) -> Selector: TPM_ALG_AES (0x0006)
        put_u16_be(&mut cmd, idx, TPM_ALG_AES); idx += 2;
        put_u16_be(&mut cmd, idx, 256); idx += 2;          // KeyBits: 128
        put_u16_be(&mut cmd, idx, TPM_ALG_AES_CFB); idx += 2; // Mode: 0x0043

        // B. Scheme (TPMT_RSA_SCHEME) -> Selector: TPM_ALG_NULL (0x0010)
        // Important: If this is NOT 0x0010, the TPM expects details, causing Error 293.
        put_u16_be(&mut cmd, idx, TPM_ALG_NULL); idx += 2;

        // C. KeyBits (RSA)
        put_u16_be(&mut cmd, idx, 2048); idx += 2;

        // D. Exponent (0 = 65537)
        put_u32_be(&mut cmd, idx, 0); idx += 4;

        // 6. Unique (TPM2B_PUBLIC_KEY_RSA) -> Size 0
        put_u16_be(&mut cmd, idx, 0); idx += 2;

        // Patch Public Size
        let pub_size = (idx - pub_start - 2) as u16;
        put_u16_be(&mut cmd, pub_start, pub_size);

        // Outside Info + PCR
        put_u16_be(&mut cmd, idx, 0); idx += 2;
        put_u32_be(&mut cmd, idx, 0); idx += 4;

        // Finalize Header Size
        put_u32_be(&mut cmd, 2, idx as u32);

        // Send
        let mut resp = [0u8; 2048];
        let len = self.send_raw_command(&cmd[..idx], &mut resp)?;

        // Verify Success
        let rc = get_u32_be(&resp, 6);
        if rc != 0 { return Err(TpmError::ResponseError(rc as u16)); }

        // Parse Handle (Standard response: Tag(2), Size(4), RC(4), Handle(4), ...)
        let handle = get_u32_be(&resp, 10);
        Ok(handle)
    }

    /// Helper: Encrypts data (Key) with PIN using TPM2_Create
    fn tpm2_create(&self, parent_handle: u32, pin: &[u8], key: &[u8], output_pub: &mut [u8], output_priv: &mut [u8])
                   -> Result<(usize, usize), TpmError>
    {
        let mut cmd = [0u8; 512];
        let mut idx = 0;

        // Header
        put_u16_be(&mut cmd, idx, TPM2_ST_SESSIONS); idx += 2;
        idx += 4; // size
        put_u32_be(&mut cmd, idx, TPM2_CC_CREATE); idx += 4;

        // Parent Handle
        put_u32_be(&mut cmd, idx, parent_handle); idx += 4;
        // Authorization Area (Mandatory for TPM_RH_OWNER)
        put_u32_be(&mut cmd, idx, 9); idx += 4;
        put_u32_be(&mut cmd, idx, TPM_RS_PW); idx += 4;
        put_u16_be(&mut cmd, idx, 0); idx += 2;
        put_u8(&mut cmd, idx, 0x00); idx += 1;
        put_u16_be(&mut cmd, idx, 0); idx += 2;
        // --- Sensitive Data ---
        let sens_start = idx;
        idx += 2;
        // Auth (PIN)
        put_u16_be(&mut cmd, idx, pin.len() as u16); idx += 2;
        cmd[idx..idx+pin.len()].copy_from_slice(pin); idx += pin.len();
        // Data (AES Key)
        put_u16_be(&mut cmd, idx, key.len() as u16); idx += 2;
        cmd[idx..idx+key.len()].copy_from_slice(key); idx += key.len();
        // Patch Sensitive Size
        put_u16_be(&mut cmd, sens_start, (idx - sens_start - 2) as u16);

        let pub_start = idx;
        idx += 2;

        put_u16_be(&mut cmd, idx, TPM_ALG_KEYEDHASH); idx += 2;
        put_u16_be(&mut cmd, idx, TPM_ALG_SHA256); idx += 2;

        // Attributes for Sealed Data
        // 1. FixedTPM/FixedParent: Binds it to this device/hierarchy
        // 2. UserWithAuth: Allows PIN use
        // 3. NO "Restricted": Because it's data, not a parent key
        // 4. NO "SensitiveDataOrigin": Because WE provided the key (in Sensitive Data)
        // 5. NO "NoDA": We WANT dictionary attack protection
        let disk_key_attrs = TPM_SRK_ATTRS_FIXEDTPM
            | TPM_SRK_ATTRS_FIXEDPARENT
            | TPM_SRK_ATTRS_USERWITHAUTH;

        put_u32_be(&mut cmd, idx, disk_key_attrs); idx += 4;
        put_u16_be(&mut cmd, idx, 0); idx += 2; // authPolicy

        // KeyedHash Parameters
        put_u16_be(&mut cmd, idx, TPM_ALG_NULL); idx += 2; // scheme
        put_u16_be(&mut cmd, idx, 0); idx += 2; // unique

        // Patch Public Size
        put_u16_be(&mut cmd, pub_start, (idx - pub_start - 2) as u16);

        // Outside Info + PCR
        put_u16_be(&mut cmd, idx, 0); idx += 2;
        put_u32_be(&mut cmd, idx, 0); idx += 4;

        // Finalize Header Size
        put_u32_be(&mut cmd, 2, idx as u32);

        // Send & Parse
        let mut resp = [0u8; 2048];
        self.send_raw_command(&cmd[..idx], &mut resp)?;

        let rc = get_u32_be(&resp, 6);
        if rc != 0 { return Err(TpmError::ResponseError(rc as u16)); }

        let resp_tag = get_u16_be(&resp, 0);
        // If tag is TPM_ST_SESSIONS (0x8002), skip 'Parameter Size' (4 bytes)
        let mut r_idx = if resp_tag == TPM2_ST_SESSIONS { 14 } else { 10 };

        // 1. outPrivate
        let priv_size = get_u16_be(&resp, r_idx) as usize; r_idx += 2;
        if output_priv.len() < priv_size { return Err(TpmError::BufferTooSmall); }
        output_priv[..priv_size].copy_from_slice(&resp[r_idx..r_idx+priv_size]);
        r_idx += priv_size;

        // 2. outPublic
        let pub_size = get_u16_be(&resp, r_idx) as usize; r_idx += 2;
        if output_pub.len() < pub_size { return Err(TpmError::BufferTooSmall); }
        output_pub[..pub_size].copy_from_slice(&resp[r_idx..r_idx+pub_size]);

        Ok((pub_size, priv_size))
    }

    pub fn decrypt_master_key(&self, pin: &[u8], input_pub: &[u8], input_priv: &[u8], output_key: &mut [u8])
                              -> Result<usize, UnlockError>
    {
        use log::{info, debug, warn};

        let srk_handle = self.create_primary_srk().map_err(UnlockError::TpmError)?;
        debug!("[FDE] SRK Loaded. Handle: 0x{:08x}", srk_handle);

        let object_handle = match self.tpm2_load(srk_handle, input_pub, input_priv) {
            Ok(h) => h,
            Err(e) => {
                self.flush_context(srk_handle).ok();
                return Err(UnlockError::TpmError(e));
            }
        };
        debug!("[FDE] Sealed Object Loaded. Handle: 0x{:08x}", object_handle);

        let result = self.tpm2_unseal(object_handle, pin, output_key);

        self.flush_context(object_handle).ok();
        self.flush_context(srk_handle).ok();

        match result {
            Ok(sz) => {
                info!("[FDE] Success: Master key unsealed.");
                Ok(sz)
            },
            Err(TpmError::ResponseError(rc)) => {

                if rc & TPM_RC_LOCKOUT == TPM_RC_LOCKOUT {
                    warn!("[FDE] FATAL: TPM is in Lockout mode!");
                    return Err(UnlockError::Lockout);
                }
                if rc & TPM_RC_AUTH_FAIL == TPM_RC_AUTH_FAIL {
                    warn!("[FDE] Auth Failed. Wrong PIN");
                    return Err(UnlockError::WrongPin);
                }
                return Err(UnlockError::TpmError(TpmError::ResponseError(rc)));
            },
            Err(e) => Err(UnlockError::TpmError(e)),
        }
    }
    fn tpm2_load(&self, parent_handle: u32, pub_blob: &[u8], priv_blob: &[u8]) -> Result<u32, TpmError> {
        let mut cmd = [0u8; 2048];
        let mut idx = 0;

        // Header
        put_u16_be(&mut cmd, idx, TPM2_ST_SESSIONS); idx += 2;
        idx += 4; // size
        put_u32_be(&mut cmd, idx, TPM2_CC_LOAD); idx += 4;

        // Parent Handle
        put_u32_be(&mut cmd, idx, parent_handle); idx += 4;
        // Authorization Area
        put_u32_be(&mut cmd, idx, 9); idx += 4;
        put_u32_be(&mut cmd, idx, TPM_RS_PW); idx += 4;
        put_u16_be(&mut cmd, idx, 0); idx += 2;
        put_u8(&mut cmd, idx, 0x00); idx += 1;
        put_u16_be(&mut cmd, idx, 0); idx += 2;

        // Private
        put_u16_be(&mut cmd, idx, priv_blob.len() as u16); idx += 2;
        cmd[idx..idx+priv_blob.len()].copy_from_slice(priv_blob); idx += priv_blob.len();

        // Public
        put_u16_be(&mut cmd, idx, pub_blob.len() as u16); idx += 2;
        cmd[idx..idx+pub_blob.len()].copy_from_slice(pub_blob); idx += pub_blob.len();

        // Patch Command Size
        put_u32_be(&mut cmd, 2, idx as u32);

        // Execute
        let mut resp = [0u8; 64];
        self.send_raw_command(&cmd[..idx], &mut resp)?;

        let rc = get_u32_be(&resp, 6);
        if rc != 0 { return Err(TpmError::ResponseError(rc as u16)); }

        // Return the new Object Handle
        Ok(get_u32_be(&resp, 10))
    }

    /// Helper: Unseals data using Password Authorization
    fn tpm2_unseal(&self, item_handle: u32, pin: &[u8], output: &mut [u8]) -> Result<usize, TpmError> {
        let mut cmd = [0u8; 512];
        let mut idx = 0;

        put_u16_be(&mut cmd, idx, TPM2_ST_SESSIONS); idx += 2;
        idx += 4; // size
        put_u32_be(&mut cmd, idx, TPM2_CC_UNSEAL); idx += 4;

        // Parent Handle
        put_u32_be(&mut cmd, idx, item_handle); idx += 4;

        // Authorization Area
        let auth_size_idx = idx;
        idx += 4;
        put_u32_be(&mut cmd, idx, TPM_RS_PW); idx += 4;
        put_u16_be(&mut cmd, idx, 0); idx += 2;
        // Session Attributes (ContinueSession=1) -> 0x01
        put_u8(&mut cmd, idx, 0x01); idx += 1;
        // HMAC (This is where the Password/PIN goes)
        put_u16_be(&mut cmd, idx, pin.len() as u16); idx += 2;
        cmd[idx..idx+pin.len()].copy_from_slice(pin); idx += pin.len();

        // Patch Authorization Size (Current idx - Start of Session Data)
        let auth_len = idx - (auth_size_idx + 4);
        put_u32_be(&mut cmd, auth_size_idx, auth_len as u32);

        // Patch Command Size
        put_u32_be(&mut cmd, 2, idx as u32);

        // Execute
        let mut resp = [0u8; 256];
        self.send_raw_command(&cmd[..idx], &mut resp)?;

        // Check Response
        let rc = get_u32_be(&resp, 6);
        if rc != 0 { return Err(TpmError::ResponseError(rc as u16)); }
        let resp_tag = get_u16_be(&resp, 0);
        // If tag is TPM_ST_SESSIONS (0x8002), skip 'Parameter Size' (4 bytes)
        let mut r_idx = if resp_tag == TPM2_ST_SESSIONS { 14 } else { 10 };

        // TPM2B_SENSITIVE_DATA
        let data_size = get_u16_be(&resp, r_idx) as usize; r_idx += 2;

        if output.len() < data_size {
            return Err(TpmError::BufferTooSmall);
        }
        output[..data_size].copy_from_slice(&resp[r_idx..r_idx+data_size]);
        Ok(data_size)
    }

    fn flush_context(&self, handle: u32) -> Result<(), TpmError> {
        let mut cmd = [0u8; 14];
        put_u16_be(&mut cmd, 0, TPM2_ST_NO_SESSIONS);
        put_u32_be(&mut cmd, 2, 14);
        put_u32_be(&mut cmd, 6, TPM2_CC_FLUSH_CONTEXT);
        put_u32_be(&mut cmd, 10, handle);

        let mut resp = [0u8; 16];
        self.send_raw_command(&cmd, &mut resp)?;
        Ok(())
    }

    pub fn test_tpm_key(&self) -> Result<(), UnlockError> {
        use log::debug;
        let pin = b"123456";
        let mut pub_blob = [0u8; 512];
        let mut priv_blob = [0u8; 512];
        let mut master_key = [0u8; 64];
        self.get_random_number(&mut master_key).map_err(UnlockError::TpmError)?;
        debug!("Master key: {:?}", master_key);
        let (pub_size, priv_size) =self.provision_master_key(pin, &master_key, &mut pub_blob, &mut priv_blob).map_err(UnlockError::TpmError)?;
        debug!("Master key provisioned with public size {} and private size {}", pub_size, priv_size);
        let mut decrypted_key = [0u8; 32];
        let pin2 = b"123654";
        let sz = self.decrypt_master_key(pin2, &pub_blob[..pub_size], &priv_blob[..priv_size], &mut decrypted_key)?;
        debug!("Decrypted master key: {:?}", decrypted_key);
        if sz != decrypted_key.len() || decrypted_key[..sz] != master_key[..sz] {
            debug!("[TPM] Test Key Decryption Failed");
            return Err(UnlockError::WrongPin);
        } else {
            debug!("[TPM] Test Key Decryption Succeeded");
            Ok(())
        }
    }

    pub fn tpm_startup(&self) -> Result<(),TpmError>{
        use log::{debug,info};

        debug!("[TPM] Sending TPM2_Startups(CLEAR)");

        let mut command_buffer = [0u8; 12];
        let commlen = command_buffer.len();

        put_u16_be(&mut command_buffer, 0, TPM2_ST_NO_SESSIONS);
        // length
        put_u32_be(&mut command_buffer, 2, commlen as u32);
        // command code
        put_u32_be(&mut command_buffer, 6, TPM2_CC_STARTUP);
        // startup type (TPM2_SU_CLEAR)
        put_u16_be(&mut command_buffer, 10, TPM2_SU_CLEAR);

        debug!("[TPM] Startup command buffer: {:02x?}" , &command_buffer);

        let mut response = [0u8; 64];
        let response_length = self.send_raw_command(&command_buffer, &mut response)?;

        if response_length < 10 {
            return Err(TpmError::BufferTooSmall);
        }

        let response_code = get_u32_be(&response, 6);

        if response_code !=0 {
            debug!("[TPM] Startup error 0x{:08x}",response_code);

            if response_code == 0x100{
                debug!("[TPM] TPM already initialized");
                return Ok(())
            }
            return Err(TpmError::ResponseError((response_code & 0xFFFF) as u16));
        }
        
        debug!("[TPM] Startup Successful");
        Ok(())


    }


}




pub fn init_tpm() -> TpmDevice{ // Device Id, Vendor Id and Revision Id.
    use log::info;

    let device = TpmDevice::new(TPM_TIS_BASE);
    info!("[TPM] Initializing tpm at 0x{:x}", TPM_TIS_BASE);

    let (did, vid) = device.read_did_vid();
    info!("[TPM] Device id 0x{:x}",did);

    if did == 0x0000 || did == 0xffff || vid == 0x0000 || vid == 0xffff {
        info!("[TPM] ERROR: TPM device not responding! DID=0x{:x}, VID=0x{:x}", did, vid);
    }

    device.request_locality_0();
    let locality_ok = device.request_locality_0();
    info!("[TPM] Locality 0: {}", if locality_ok { "OK" } else { "FAILED" });

    let cmd_ready = device.command_ready();
    info!("[TPM] Command ready: {}", if cmd_ready { "OK" } else { "FAILED" });
    
    match device.tpm_startup() {
        Ok(_) => info!("[TPM] Startup successful"),
        Err(e) => info!("[TPM] Startup failed: {:?}", e),
    }

    let sts = device.read_sts();
    info!("[TPM] Final STS: 0x{:02x}", sts);

    return device
}

//Helper functions. Section written by AI
fn put_u8(buf: &mut [u8], offset: usize, value: u8) {
    buf[offset] = value;
}
fn put_u16_be(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset]     = (value >> 8) as u8;
    buf[offset + 1] = (value & 0xFF) as u8;
}

fn put_u32_be(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset]     = (value >> 24) as u8;
    buf[offset + 1] = (value >> 16) as u8;
    buf[offset + 2] = (value >> 8) as u8;
    buf[offset + 3] = (value & 0xFF) as u8;
}

fn get_u16_be(buf: &[u8], offset: usize) -> u16 {
    ((buf[offset] as u16) << 8) | (buf[offset + 1] as u16)
}

fn get_u32_be(buf: &[u8], offset: usize) -> u32 {
    ((buf[offset] as u32) << 24)
        | ((buf[offset + 1] as u32) << 16)
        | ((buf[offset + 2] as u32) << 8)
        | (buf[offset + 3] as u32)
}
//End of written by AI section