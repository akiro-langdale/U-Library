//! C ABI for process virtual memory.

use super::INVALID_ARGUMENT;
#[cfg(windows)]
use super::{INTERNAL_ERROR, OK};
#[cfg(windows)]
use std::panic::catch_unwind;
use std::ptr;

#[cfg(not(windows))]
use super::UNSUPPORTED_SOURCE;
#[cfg(windows)]
use super::{INVALID_RANGE, IO_ERROR};
#[cfg(windows)]
use u_library_memory::process_memory::{
    MemoryRegionKind, MemoryState, ProcessMemory, ProcessMemoryError, VirtualMemoryRegion,
    MAX_MEMORY_SAMPLE_BYTES,
};

#[cfg(windows)]
const BUFFER_TOO_SMALL: i32 = 6;
#[cfg(windows)]
const PERMISSION_REQUIRED: i32 = 7;
#[cfg(windows)]
const UNAVAILABLE: i32 = 8;

pub struct ProcessHandle {
    #[cfg(windows)]
    process: ProcessMemory,
}

#[repr(C)]
pub struct ProcessRegion {
    base_address: u64,
    size_bytes: u64,
    state: u32,
    kind: u32,
    flags: u32,
    reserved: u32,
}

#[cfg(windows)]
impl From<VirtualMemoryRegion> for ProcessRegion {
    fn from(region: VirtualMemoryRegion) -> Self {
        let state = match region.state {
            MemoryState::Unknown => 0,
            MemoryState::Free => 1,
            MemoryState::Reserved => 2,
            MemoryState::Committed => 3,
        };
        let kind = match region.kind {
            MemoryRegionKind::Unknown => 0,
            MemoryRegionKind::Free => 1,
            MemoryRegionKind::Private => 2,
            MemoryRegionKind::Image => 3,
            MemoryRegionKind::Mapped => 4,
        };
        let protection = region.protection;
        let flags = u32::from(protection.read)
            | (u32::from(protection.write) << 1)
            | (u32::from(protection.execute) << 2)
            | (u32::from(protection.guard) << 3)
            | (u32::from(region.readable) << 4);
        Self {
            base_address: region.base_address,
            size_bytes: region.size_bytes,
            state,
            kind,
            flags,
            reserved: 0,
        }
    }
}

#[cfg(windows)]
fn status(error: ProcessMemoryError) -> i32 {
    match error {
        ProcessMemoryError::InvalidRange(_) => INVALID_RANGE,
        ProcessMemoryError::PermissionRequired(_) => PERMISSION_REQUIRED,
        ProcessMemoryError::Unavailable(_) => UNAVAILABLE,
        ProcessMemoryError::EnumerationLimit(_) => IO_ERROR,
    }
}

#[no_mangle]
pub extern "C" fn ul_process_abi_version() -> u32 {
    u32::from(cfg!(windows))
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_process_open(pid: u64, out: *mut *mut ProcessHandle) -> i32 {
    if out.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { out.write(ptr::null_mut()) };
    #[cfg(windows)]
    {
        catch_unwind(|| match ProcessMemory::open(pid) {
            Ok(process) => {
                unsafe { out.write(Box::into_raw(Box::new(ProcessHandle { process }))) };
                OK
            }
            Err(error) => status(error),
        })
        .unwrap_or(INTERNAL_ERROR)
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        UNSUPPORTED_SOURCE
    }
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_process_regions(
    handle: *const ProcessHandle,
    buffer: *mut ProcessRegion,
    capacity: usize,
    count: *mut usize,
) -> i32 {
    if count.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { count.write(0) };
    if handle.is_null() || (capacity != 0 && buffer.is_null()) {
        return INVALID_ARGUMENT;
    }
    #[cfg(windows)]
    {
        if capacity > isize::MAX as usize / std::mem::size_of::<ProcessRegion>() {
            return INVALID_RANGE;
        }
        catch_unwind(|| match unsafe { &*handle }.process.regions() {
            Ok(regions) => {
                unsafe { count.write(regions.len()) };
                if capacity < regions.len() {
                    return BUFFER_TOO_SMALL;
                }
                for (index, region) in regions.into_iter().enumerate() {
                    unsafe { buffer.add(index).write(region.into()) };
                }
                OK
            }
            Err(error) => status(error),
        })
        .unwrap_or(INTERNAL_ERROR)
    }
    #[cfg(not(windows))]
    UNSUPPORTED_SOURCE
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_process_read_at(
    handle: *const ProcessHandle,
    address: u64,
    buffer: *mut u8,
    capacity: usize,
    bytes_read: *mut usize,
) -> i32 {
    if bytes_read.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { bytes_read.write(0) };
    if handle.is_null() || buffer.is_null() {
        return INVALID_ARGUMENT;
    }
    #[cfg(windows)]
    {
        if !(1..=MAX_MEMORY_SAMPLE_BYTES).contains(&capacity) {
            return INVALID_RANGE;
        }
        catch_unwind(
            || match unsafe { &*handle }.process.read(address, capacity) {
                Ok(bytes) => {
                    unsafe {
                        ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
                        bytes_read.write(bytes.len());
                    }
                    OK
                }
                Err(error) => status(error),
            },
        )
        .unwrap_or(INTERNAL_ERROR)
    }
    #[cfg(not(windows))]
    {
        let _ = (address, capacity);
        UNSUPPORTED_SOURCE
    }
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_process_close(handle: *mut ProcessHandle) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) };
    }
}
