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


pub struct TpmDevice(usize);

impl TpmDevice {
    pub fn new(base_addr: usize) -> Self {
        TpmDevice(base_addr)
    }
}

Enum TpmError{
    Timeout,
    BufferTooSmall,
    NotReady,

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
        self.Write_command_to_fifo(command)?;
        
        self.start_execution();

        let num = self.read_response_from_fifo(response)?;

        Ok(num)
    }

    fn start_execution(&self) {
            self.write_sts(TPM_STS_TPM_GO);
        }

    fn Write_command_to_fifo(&self, command: &[u8]) -> Result<(), TpmError> {
        let sts = self.read_sts();
        let ready = (sts & TPM_STS_COMMAND_READY) != 0;
        if !ready {
            return Err(TpmError::NotReady);
        }

        for &b in command{
            let mut ok = false;

            for _ in 0..1000{ //arbitrary number just dont want it to freeze with a while loop
                let status_now = self.read_sts();
                if (status_now & TPM_STS_EXPECT) != 0 {
                    ok = true;
                    break;
                }
            }
            if !ok{
                return Err(TpmError::Timeout)
            }
            self.fifo_write_byte(b)
        }
        Ok (())
    }

    fn read_response_from_fifo(&self, response: &mut [u8]) -> Result<usize, TpmError>{
        let mut ok = false
        for _ in 0..1000{
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
            let did_vid = did_vid_ptr().read_volatile();
            let did = (did_vid >> 16) as u16;
            let vid = (did_vid & 0xFFFF) as u16;
            (did, vid)
        }
    }

    pub fn read_rid(&self) -> u8 {
        unsafe { self.rid_ptr().read_volatile() }
    }

    fn read_access(&self) -> u8{
        unsafe { self.access_ptr().read_volatile())}
    }

    fn write_access(&self) -> u8{
        unsafe { self.access_ptr().write_volatile(value))}
    }

    fn read_sts(&self) -> u8{
        unsafe { self.sts_ptr().read_volatile())}
    }

    fn write_sts(&self) -> u8{
        unsafe { self.sts_ptr().write_volatile(value))}
    }

    pub fn request_locality_0(&self) -> bool{
        self.write_access(TPM_ACCESS_REQUEST_USE);

        //poll until valid + active locailty bits are set
        for _ in 0..1_000{ //arbitrary number just dont want it to freeze with a while loop
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

        self.write_sts(TPM_STS_COMMAND_READY);

        for _ in 0..1_000 { //arbitrary number just dont want it to freeze with a while loop
            let sts = self.read_sts();
            let valid = (sts & TPM_STS_VALID) != 0;
            let ready = (sts & TPM_STS_COMMAND_READY) != 0;

            if valid && ready {
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
}

pub fn init_tpm() -> TpmDevice{
    let device = TpmDevice::new(TPM_TIS_BASE);

    let (did, vid) = dev.read_did_vid();
    let rid = dev.read_rid();

    device.request_locality_0();
    device.command_ready();

    return device
}