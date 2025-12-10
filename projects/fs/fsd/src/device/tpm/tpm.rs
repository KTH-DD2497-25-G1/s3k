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
const TPM2_CC_GET_RANDOM:  u32 = 0x0000017B;

const TPM2_CC_STARTUP: u32 = 0x00000144;
const TPM2_SU_CLEAR: u16 = 0x0000;


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

        self.Write_command_to_fifo(command)?;
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

    fn Write_command_to_fifo(&self, command: &[u8]) -> Result<(), TpmError> {
        use log::debug;

        let sts = self.read_sts();
        let ready = (sts & TPM_STS_COMMAND_READY) != 0;
        if !ready {
            debug!("[TPM] Not ready, STS: 0x{:x}", sts);
            return Err(TpmError::NotReady);
        }

        debug!("[TPM] STS ready, writing {} bytes to FIFO", command.len());

        for (i, &b) in command.iter().enumerate() {
            self.fifo_write_byte(b);
            debug!("[TPM] Wrote byte {} of {}", i + 1, command.len());
        }
    
        debug!("[TPM] All bytes written to FIFO");
        Ok(())
    }

    fn read_response_from_fifo(&self, response: &mut [u8]) -> Result<usize, TpmError>{
        let mut ok = false;
        for _ in 0..1_000_000{
            let status = self.read_sts();
            if (status & TPM_STS_DATA_AVAIL) !=0 {
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
            if (status & TPM_STS_DATA_AVAIL) == 0 {
                break;
            }
        }
        Ok(index)
    }

    fn access_ptr(&self) -> *mut u8 {
        (self.0 + TPM_ACCESS).as_ptr_u8()
    }

    fn sts_ptr(&self) -> *mut u8 {
        (self.0 + TPM_STS).as_ptr_u8()
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

    fn read_sts(&self) -> u8{
        unsafe { self.sts_ptr().read_volatile()}
    }

    fn write_sts(&self, value: u8){
        unsafe { self.sts_ptr().write_volatile(value)}
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
            let ready = (sts & TPM_STS_COMMAND_READY) != 0;

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
        let mut response = [0u8;64]; //64 bytes should be big enough for GetRandom

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
        // Continue anyway for now, but this is the problem
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