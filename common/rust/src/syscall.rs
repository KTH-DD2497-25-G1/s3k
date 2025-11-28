use core::arch::asm;
use crate::ffi::{S3kCap, S3kCidx, S3kErr, S3kMsg, S3kPid, S3kPmpSlot, S3kReg, S3kReply, S3kState};

#[repr(u64)]
pub enum Syscall {
    // Basic Info & Registers
    GetInfo = 0,  // Retrieve system information
    RegRead = 1,  // Read from a register
    RegWrite = 2, // Write to a register
    Sync = 3,     // Synchronize memory and time.
    Sleep = 4,

    // Capability Management
    CapRead = 5,   // Read the properties of a capability.
    CapMove = 6,   // Move a capability to a different slot.
    CapDelete = 7, // Delete a capability from the system.
    CapRevoke = 8, // Deletes derived capabilities.
    CapDerive = 9, // Creates a new capability.

    // PMP calls
    PmpLoad = 10,
    PmpUnload = 11,

    // Monitor calls
    MonSuspend = 12,
    MonResume = 13,
    MonStateGet = 14,
    MonYield = 15,
    MonRegRead = 16,
    MonRegWrite = 17,
    MonCapRead = 18,
    MonCapMove = 19,
    MonPmpLoad = 20,
    MonPmpUnload = 21,

    // Socket calls
    SockSend = 22,
    SockRecv = 23,
    SockSendRecv = 24,
}

macro_rules! syscall {
    ($code:expr) => {{
        let t0: u64;
        unsafe {
            asm!("ecall", inout("t0") $code as u64 => t0);
        }
        t0
    }};
    ($code:expr, $arg0: expr) => {{
        let t0: u64;
        let a0: u64;
        unsafe {
            asm!("ecall", inout("t0") $code as u64 => t0, inout("a0") $arg0 => a0);
        }
        Ret(S3kErr::from(t0 as u8), a0).into_result()
    }};
    ($code: expr, $arg0: expr, $arg1: expr) => {{
        let t0: u64;
        let a0: u64;
        unsafe {
            asm!("ecall", inout("t0") $code as u64 => t0, inout("a0") $arg0 => a0, in("a1") $arg1);
        }
        Ret(S3kErr::from(t0 as u8), a0).into_result()
    }};
    ($code: expr, $arg0: expr, $arg1: expr, $arg2: expr) => {{
        let t0: u64;
        let a0: u64;
        unsafe {
            asm!("ecall", inout("t0") $code as u64 => t0, inout("a0") $arg0 => a0, in("a1") $arg1, in("a2") $arg2);
        }
        Ret(S3kErr::from(t0 as u8), a0).into_result()
    }};
    ($code: expr, $arg0: expr, $arg1: expr, $arg2: expr, $arg3: expr) => {{
        let t0: u64;
        let a0: u64;
        unsafe {
            asm!("ecall", inout("t0") $code as u64 => t0, inout("a0") $arg0 => a0, in("a1") $arg1, in("a2") $arg2, in("a3") $arg3);
        }
        Ret(S3kErr::from(t0 as u8), a0).into_result()
    }};
    ($code: expr, $arg0: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr) => {{
        let t0: u64;
        let a0: u64;
        unsafe {
            asm!("ecall", inout("t0") $code as u64 => t0, inout("a0") $arg0 => a0, in("a1") $arg1, in("a2") $arg2, in("a3") $arg3, in("a4") $arg4);
        }
        Ret(S3kErr::from(t0 as u8), a0).into_result()
    }};
    ($code: expr, $arg0: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr, $arg5: expr) => {{
        let t0: u64;
        let a0: u64;
        unsafe {
            asm!("ecall", inout("t0") $code as u64 => t0, inout("a0") $arg0 => a0, in("a1") $arg1, in("a2") $arg2, in("a3") $arg3, in("a4") $arg4, in("a5") $arg5);
        }
        Ret(S3kErr::from(t0 as u8), a0).into_result()
    }};
    ($code: expr, $arg0: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr, $arg5: expr, $arg6: expr) => {{
        let t0: u64;
        let a0: u64;
        unsafe {
            asm!("ecall", inout("t0") $code as u64 => t0, inout("a0") $arg0 => a0, in("a1") $arg1, in("a2") $arg2, in("a3") $arg3, in("a4") $arg4, in("a5") $arg5, in("a6") $arg6);
        }
        Ret(S3kErr::from(t0 as u8), a0).into_result()
    }};
    ($code: expr, $arg0: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr, $arg5: expr, $arg6: expr, $arg7: expr) => {{
        let t0: u64;
        let a0: u64;
        unsafe {
            asm!("ecall", inout("t0") $code as u64 => t0, inout("a0") $arg0 => a0, in("a1") $arg1, in("a2") $arg2, in("a3") $arg3, in("a4") $arg4, in("a5") $arg5, in("a6") $arg6, in("a7") $arg7);
        }
        Ret(S3kErr::from(t0 as u8), a0).into_result()
    }};
}

// For syscalls that return multiple values (e.g., sock_recv, sock_sendrecv)
macro_rules! syscall_asm_multi_out {
    ($code:expr, $arg0: expr, $arg1: expr, $arg2: expr, $arg3: expr, $arg4: expr, $arg5: expr, $arg6: expr, $arg7: expr) => {{
        let t0: u64;
        let a0: u64;
        let a1: u64;
        let a2: u64;
        let a3: u64;
        let a4: u64;
        let a5: u64;
        unsafe {
            asm!("ecall",
                inout("t0") $code => t0,
                inout("a0") $arg0 => a0,
                inout("a1") $arg1 => a1,
                inout("a2") $arg2 => a2,
                inout("a3") $arg3 => a3,
                inout("a4") $arg4 => a4,
                inout("a5") $arg5 => a5,
                in("a6") $arg6,
                in("a7") $arg7
            );
        }
        (t0, a0, a1, a2, a3, a4, a5)
    }};
}

#[repr(C)]
pub struct Ret(S3kErr, u64);

impl Ret {
    fn into_result(self) -> Result<u64> {
        if self.0 == S3kErr::Success {
            Ok(self.1)
        } else {
            Err(self.0)
        }
    }
}

type Result<T> = core::result::Result<T, S3kErr>;

pub fn s3k_get_pid() -> Result<u64> {
    syscall!(Syscall::GetInfo, 0u64)
}

pub fn s3k_get_hartid() -> Result<u64> {
    syscall!(Syscall::GetInfo, 1u64)
}

pub fn s3k_get_time() -> Result<u64> {
    syscall!(Syscall::GetInfo, 2u64)
}

pub fn s3k_reg_read(reg: u64) -> Result<u64> {
    syscall!(Syscall::RegRead, reg)
}

pub fn s3k_reg_write(reg: S3kReg, val: u64) -> Result<u64> {
    syscall!(Syscall::RegWrite, reg as u64, val)
}

pub fn s3k_sync_mem() {
    syscall!(Syscall::Sync, 1u64);
}

pub fn s3k_sync() {
    syscall!(Syscall::Sync, 0u64);
}

pub fn s3k_sleep(time: u64) {
    syscall!(Syscall::Sleep, time);
}

pub fn s3k_cap_read<C>(idx: S3kCidx) -> Result<C>
where
    C: S3kCap,
{
    let mut cap = 0u64;
    syscall!(Syscall::CapRead, idx as u64, &mut cap as *mut _ as u64)
        .and_then(|_| unsafe { C::from_raw(cap) })
}

pub fn s3k_try_cap_move(src: S3kCidx, dest: S3kCidx) -> Result<()> {
    syscall!(Syscall::CapMove, src as u64, dest as u64).map(|_| ())
}

pub fn s3k_cap_move(src: S3kCidx, dest: S3kCidx) -> Result<()> {
    loop {
        let res = s3k_try_cap_move(src, dest);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_cap_delete(idx: S3kCidx) -> Result<()> {
    syscall!(Syscall::CapDelete, idx as u64).map(|_| ())
}

pub fn s3k_cap_delete(idx: S3kCidx) -> Result<()> {
    loop {
        let res = s3k_try_cap_delete(idx);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_cap_revoke(idx: S3kCidx) -> Result<()> {
    syscall!(Syscall::CapRevoke, idx as u64).map(|_| ())
}

pub fn s3k_cap_revoke(idx: S3kCidx) -> Result<()> {
    loop {
        let res = s3k_try_cap_revoke(idx);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_cap_derive<C>(src: S3kCidx, dest: S3kCidx, new_cap: C) -> Result<()>
where C: S3kCap {
    syscall!(Syscall::CapDerive, src as u64, dest as u64, new_cap.as_raw()).map(|_| ())
}

pub fn s3k_cap_derive<C>(src: S3kCidx, dest: S3kCidx, new_cap: C) -> Result<()>
where C: S3kCap {
    loop {
        let res = s3k_try_cap_derive(src, dest, new_cap);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_pmp_load(idx: S3kCidx, slot: S3kPmpSlot) -> Result<()> {
    syscall!(Syscall::PmpLoad, idx as u64, slot as u64).map(|_| ())
}

pub fn s3k_pmp_load(idx: S3kCidx, slot: S3kPmpSlot) -> Result<()> {
    loop {
        let res = s3k_try_pmp_load(idx, slot);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_pmp_unload(idx: S3kCidx) -> Result<()> {
    syscall!(Syscall::PmpUnload, idx as u64).map(|_| ())
}

pub fn s3k_pmp_unload(idx: S3kCidx) -> Result<()> {
    loop {
        let res = s3k_try_pmp_unload(idx);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_suspend(mon_idx: S3kCidx, pid: S3kPid) -> Result<()> {
    syscall!(Syscall::MonSuspend, mon_idx as u64, pid as u64).map(|_| ())
}

pub fn s3k_mon_suspend(mon_idx: S3kCidx, pid: S3kPid) -> Result<()> {
    loop {
        let res = s3k_try_mon_suspend(mon_idx, pid);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_resume(mon_idx: S3kCidx, pid: S3kPid) -> Result<()> {
    syscall!(Syscall::MonResume, mon_idx as u64, pid as u64).map(|_| ())
}

pub fn s3k_mon_resume(mon_idx: S3kCidx, pid: S3kPid) -> Result<()> {
    loop {
        let res = s3k_try_mon_resume(mon_idx, pid);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_state_get(mon_idx: S3kCidx, pid: S3kPid) -> Result<S3kState> {
    syscall!(Syscall::MonStateGet, mon_idx as u64, pid as u64)
}

pub fn s3k_mon_state_get(mon_idx: S3kCidx, pid: S3kPid) -> Result<S3kState> {
    loop {
        let res = s3k_try_mon_state_get(mon_idx, pid);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_yield(mon_idx: S3kCidx, pid: S3kPid) -> Result<()> {
    syscall!(Syscall::MonYield, mon_idx as u64, pid as u64).map(|_| ())
}

pub fn s3k_mon_yield(mon_idx: S3kCidx, pid: S3kPid) -> Result<()> {
    loop {
        let res = s3k_try_mon_yield(mon_idx, pid);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_reg_read(mon_idx: S3kCidx, pid: S3kPid, reg: S3kReg) -> Result<u64> {
    syscall!(Syscall::MonRegRead, mon_idx as u64, pid as u64, reg as u64)
}

pub fn s3k_mon_reg_read(mon_idx: S3kCidx, pid: S3kPid, reg: S3kReg) -> Result<u64> {
    loop {
        let res = s3k_try_mon_reg_read(mon_idx, pid, reg);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_reg_write(mon_idx: S3kCidx, pid: S3kPid, reg: S3kReg, val: u64) -> Result<()> {
    syscall!(Syscall::MonRegWrite, mon_idx as u64, pid as u64, reg as u64, val).map(|_| ())
}

pub fn s3k_mon_reg_write(mon_idx: S3kCidx, pid: S3kPid, reg: S3kReg, val: u64) -> Result<()> {
    loop {
        let res = s3k_try_mon_reg_write(mon_idx, pid, reg, val);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_cap_read(mon_idx: S3kCidx, pid: S3kPid, idx: S3kCidx) -> Result<u64> {
    syscall!(Syscall::MonCapRead, mon_idx as u64, pid as u64, idx as u64)
}

pub fn s3k_mon_cap_read(mon_idx: S3kCidx, pid: S3kPid, idx: S3kCidx) -> Result<u64> {
    loop {
        let res = s3k_try_mon_cap_read(mon_idx, pid, idx);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_cap_move(
    mon_idx: S3kCidx,
    src_pid: S3kPid,
    src_idx: S3kCidx,
    dst_pid: S3kPid,
    dst_idx: S3kCidx,
) -> Result<()> {
    syscall!(Syscall::MonCapMove, mon_idx as u64, src_pid as u64, src_idx as u64, dst_pid as u64, dst_idx as u64).map(|_| ())
}

pub fn s3k_mon_cap_move(
    mon_idx: S3kCidx,
    src_pid: S3kPid,
    src_idx: S3kCidx,
    dst_pid: S3kPid,
    dst_idx: S3kCidx,
) -> Result<()> {
    loop {
        let res = s3k_try_mon_cap_move(mon_idx, src_pid, src_idx, dst_pid, dst_idx);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_pmp_load(
    mon_idx: S3kCidx,
    pid: S3kPid,
    pmp_idx: S3kCidx,
    pmp_slot: S3kPmpSlot,
) -> Result<()> {
    syscall!(Syscall::MonPmpLoad, mon_idx as u64, pid as u64, pmp_idx as u64, pmp_slot as u64).map(|_| ())
}

pub fn s3k_mon_pmp_load(
    mon_idx: S3kCidx,
    pid: S3kPid,
    pmp_idx: S3kCidx,
    pmp_slot: S3kPmpSlot,
) -> Result<()> {
    loop {
        let res = s3k_try_mon_pmp_load(mon_idx, pid, pmp_idx, pmp_slot);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_mon_pmp_unload(mon_idx: S3kCidx, pid: S3kPid, pmp_idx: S3kCidx) -> Result<()> {
    syscall!(Syscall::MonPmpUnload, mon_idx as u64, pid as u64, pmp_idx as u64).map(|_| ())
}

pub fn s3k_mon_pmp_unload(mon_idx: S3kCidx, pid: S3kPid, pmp_idx: S3kCidx) -> Result<()> {
    loop {
        let res = s3k_try_mon_pmp_unload(mon_idx, pid, pmp_idx);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_sock_send(sock_idx: S3kCidx, msg: &S3kMsg) -> Result<()> {
    syscall!(
        Syscall::SockSend,
        sock_idx as u64,
        msg.cap_idx as u64,
        msg.send_cap as u64,
        msg.data[0],
        msg.data[1],
        msg.data[2],
        msg.data[3]
    )
    .map(|_| ())
}

pub fn s3k_sock_send(sock_idx: S3kCidx, msg: &S3kMsg) -> Result<()> {
    loop {
        let res = s3k_try_sock_send(sock_idx, msg);
        match res {
            Err(S3kErr::Preempted) => continue,
            _ => return res,
        }
    }
}

pub fn s3k_try_sock_recv(sock_idx: S3kCidx, cap_idx: S3kCidx) -> S3kReply {
    let (t0, a0, a1, a2, a3, a4, a5) = syscall_asm_multi_out!(
        Syscall::SockRecv as u64,
        sock_idx as u64,
        cap_idx as u64,
        0u64,
        0u64,
        0u64,
        0u64,
        0u64,
        0u64
    );
    S3kReply {
        err: S3kErr::from(t0 as u8),
        tag: a0 as u32,
        cap: a1,
        data: [a2, a3, a4, a5],
    }
}

pub fn s3k_sock_recv(sock_idx: S3kCidx, cap_idx: S3kCidx) -> S3kReply {
    loop {
        let reply = s3k_try_sock_recv(sock_idx, cap_idx);
        if reply.err == S3kErr::Preempted {
            continue;
        }
        return reply;
    }
}

pub fn s3k_try_sock_sendrecv(sock_idx: S3kCidx, msg: &S3kMsg) -> S3kReply {
    let (t0, a0, a1, a2, a3, a4, a5) = syscall_asm_multi_out!(
        Syscall::SockSendRecv as u64,
        sock_idx as u64,
        msg.cap_idx as u64,
        msg.send_cap as u64,
        msg.data[0],
        msg.data[1],
        msg.data[2],
        msg.data[3],
        0u64
    );
    S3kReply {
        err: S3kErr::from(t0 as u8),
        tag: a0 as u32,
        cap: a1,
        data: [a2, a3, a4, a5],
    }
}

pub fn s3k_sock_sendrecv(sock_idx: S3kCidx, msg: &S3kMsg) -> S3kReply {
    loop {
        let reply = s3k_try_sock_sendrecv(sock_idx, msg);
        if reply.err == S3kErr::Preempted {
            continue;
        }
        return reply;
    }
}

