//! C ABI for CPU register capture.

use super::{BUFFER_TOO_SMALL, INTERNAL_ERROR, INVALID_ARGUMENT, OK, UNSUPPORTED_SOURCE};
use std::panic::catch_unwind;
use u_library_registers::{
    capture_current_thread, capture_thread, RegisterError, RegisterSnapshot,
};

const PERMISSION_REQUIRED: i32 = 7;
const UNAVAILABLE: i32 = 8;
const MAX_NAME: usize = 16;

#[repr(C)]
pub struct Register {
    name: [u8; MAX_NAME],
    bits: u64,
    width_bits: u8,
    reserved: [u8; 7],
}

fn status(error: RegisterError) -> i32 {
    match error {
        RegisterError::Unsupported(_) => UNSUPPORTED_SOURCE,
        RegisterError::PermissionRequired(_) => PERMISSION_REQUIRED,
        RegisterError::Unavailable(_) => UNAVAILABLE,
    }
}

fn write_snapshot(
    snapshot: RegisterSnapshot,
    buffer: *mut Register,
    capacity: usize,
    count: *mut usize,
    architecture: *mut u32,
    thread_id: *mut u64,
) -> i32 {
    unsafe {
        count.write(snapshot.registers.len());
        architecture.write(snapshot.architecture as u32);
        thread_id.write(snapshot.thread_id);
    }
    if capacity < snapshot.registers.len() {
        return BUFFER_TOO_SMALL;
    }
    for (index, register) in snapshot.registers.into_iter().enumerate() {
        let mut name = [0u8; MAX_NAME];
        let bytes = register.name.as_bytes();
        let copy = bytes.len().min(MAX_NAME.saturating_sub(1));
        name[..copy].copy_from_slice(&bytes[..copy]);
        unsafe {
            buffer.add(index).write(Register {
                name,
                bits: register.bits,
                width_bits: register.width_bits,
                reserved: [0; 7],
            });
        }
    }
    OK
}

#[no_mangle]
pub extern "C" fn ul_registers_abi_version() -> u32 {
    u_library_registers::registers_abi_version()
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_registers_capture_current(
    buffer: *mut Register,
    capacity: usize,
    count: *mut usize,
    architecture: *mut u32,
    thread_id: *mut u64,
) -> i32 {
    if count.is_null() || architecture.is_null() || thread_id.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe {
        count.write(0);
        architecture.write(0);
        thread_id.write(0);
    }
    if capacity != 0 && buffer.is_null() {
        return INVALID_ARGUMENT;
    }
    if capacity > isize::MAX as usize / std::mem::size_of::<Register>() {
        return INVALID_ARGUMENT;
    }
    catch_unwind(|| match capture_current_thread() {
        Ok(snapshot) => write_snapshot(snapshot, buffer, capacity, count, architecture, thread_id),
        Err(error) => status(error),
    })
    .unwrap_or(INTERNAL_ERROR)
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_registers_capture_thread(
    pid: u64,
    tid: u64,
    buffer: *mut Register,
    capacity: usize,
    count: *mut usize,
    architecture: *mut u32,
    thread_id: *mut u64,
) -> i32 {
    if count.is_null() || architecture.is_null() || thread_id.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe {
        count.write(0);
        architecture.write(0);
        thread_id.write(0);
    }
    if capacity != 0 && buffer.is_null() {
        return INVALID_ARGUMENT;
    }
    if capacity > isize::MAX as usize / std::mem::size_of::<Register>() {
        return INVALID_ARGUMENT;
    }
    catch_unwind(|| match capture_thread(pid, tid) {
        Ok(snapshot) => write_snapshot(snapshot, buffer, capacity, count, architecture, thread_id),
        Err(error) => status(error),
    })
    .unwrap_or(INTERNAL_ERROR)
}
