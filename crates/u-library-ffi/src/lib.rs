mod cache;
mod gpu;
mod process;
mod registers;

use std::{ffi::c_char, panic::catch_unwind, ptr, slice, str, sync::Mutex};
use u_library_core::{
    compare_bytes, CompareError, ReadError, DEFAULT_SECTOR_SIZE, MAX_READ_BYTES,
};
use u_library_memory::FileSource;

const OK: i32 = 0;
const INVALID_ARGUMENT: i32 = 1;
const INVALID_RANGE: i32 = 2;
const UNSUPPORTED_SOURCE: i32 = 3;
const IO_ERROR: i32 = 4;
const INTERNAL_ERROR: i32 = 5;
const BUFFER_TOO_SMALL: i32 = 6;

#[repr(C)]
pub struct ByteDiff {
    address: u64,
    before: u8,
    after: u8,
    reserved: [u8; 6],
}

pub struct FileHandle(Mutex<FileSource>);

fn status(error: ReadError) -> i32 {
    match error {
        ReadError::InvalidRange => INVALID_RANGE,
        ReadError::UnsupportedSource => UNSUPPORTED_SOURCE,
        ReadError::Io(_) => IO_ERROR,
    }
}

#[no_mangle]
pub extern "C" fn ul_abi_version() -> u32 {
    1
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_bytes_compare(
    base_address: u64,
    before: *const u8,
    before_len: usize,
    after: *const u8,
    after_len: usize,
    out: *mut ByteDiff,
    capacity: usize,
    count: *mut usize,
) -> i32 {
    if count.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { count.write(0) };
    if before_len != after_len {
        return INVALID_ARGUMENT;
    }
    if before_len > MAX_READ_BYTES || base_address.checked_add(before_len as u64).is_none() {
        return INVALID_RANGE;
    }
    if (before.is_null() && before_len != 0)
        || (after.is_null() && after_len != 0)
        || (capacity != 0 && out.is_null())
    {
        return INVALID_ARGUMENT;
    }
    if capacity > isize::MAX as usize / std::mem::size_of::<ByteDiff>() {
        return INVALID_RANGE;
    }
    catch_unwind(|| {
        let before = if before_len == 0 {
            &[][..]
        } else {
            unsafe { slice::from_raw_parts(before, before_len) }
        };
        let after = if after_len == 0 {
            &[][..]
        } else {
            unsafe { slice::from_raw_parts(after, after_len) }
        };
        match compare_bytes(base_address, before, after) {
            Ok(changes) => {
                unsafe { count.write(changes.len()) };
                if capacity < changes.len() {
                    return BUFFER_TOO_SMALL;
                }
                for (index, change) in changes.into_iter().enumerate() {
                    unsafe {
                        out.add(index).write(ByteDiff {
                            address: change.address,
                            before: change.before,
                            after: change.after,
                            reserved: [0; 6],
                        });
                    }
                }
                OK
            }
            Err(CompareError::LengthMismatch) => INVALID_ARGUMENT,
            Err(CompareError::AddressMismatch | CompareError::AddressSpaceMismatch) => {
                INVALID_RANGE
            }
        }
    })
    .unwrap_or(INTERNAL_ERROR)
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_file_open(
    path: *const c_char,
    path_len: usize,
    out: *mut *mut FileHandle,
) -> i32 {
    if out.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { out.write(ptr::null_mut()) };
    if path.is_null() || path_len == 0 || path_len > isize::MAX as usize {
        return INVALID_ARGUMENT;
    }
    catch_unwind(|| {
        let bytes = unsafe { slice::from_raw_parts(path.cast::<u8>(), path_len) };
        let Ok(path) = str::from_utf8(bytes) else {
            return INVALID_ARGUMENT;
        };
        if path.contains('\0') {
            return INVALID_ARGUMENT;
        }
        match FileSource::open(path) {
            Ok(source) => {
                let handle = Box::new(FileHandle(Mutex::new(source)));
                unsafe { out.write(Box::into_raw(handle)) };
                OK
            }
            Err(error) => status(error),
        }
    })
    .unwrap_or(INTERNAL_ERROR)
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_file_read_at(
    handle: *const FileHandle,
    offset: u64,
    buffer: *mut u8,
    capacity: usize,
    bytes_read: *mut usize,
) -> i32 {
    if bytes_read.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { bytes_read.write(0) };
    if handle.is_null() || (buffer.is_null() && capacity != 0) {
        return INVALID_ARGUMENT;
    }
    if capacity > MAX_READ_BYTES || offset.checked_add(capacity as u64).is_none() {
        return INVALID_RANGE;
    }
    catch_unwind(|| {
        let handle = unsafe { &*handle };
        let Ok(mut source) = handle.0.lock() else {
            return INTERNAL_ERROR;
        };
        // C callers may supply uninitialized storage. Copy only the bytes read.
        let mut scratch = Vec::new();
        if scratch.try_reserve_exact(capacity).is_err() {
            return INTERNAL_ERROR;
        }
        scratch.resize(capacity, 0);
        match source.read_at(offset, &mut scratch) {
            Ok(receipt) => {
                unsafe {
                    if receipt.bytes_read != 0 {
                        ptr::copy_nonoverlapping(scratch.as_ptr(), buffer, receipt.bytes_read);
                    }
                    bytes_read.write(receipt.bytes_read);
                }
                OK
            }
            Err(error) => status(error),
        }
    })
    .unwrap_or(INTERNAL_ERROR)
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_file_close(handle: *mut FileHandle) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) };
    }
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_file_size(handle: *const FileHandle, out_size: *mut u64) -> i32 {
    if handle.is_null() || out_size.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { out_size.write(0) };
    catch_unwind(|| {
        let handle = unsafe { &*handle };
        let Ok(source) = handle.0.lock() else {
            return INTERNAL_ERROR;
        };
        match source.size_bytes() {
            Ok(size) => {
                unsafe { out_size.write(size) };
                OK
            }
            Err(error) => status(error),
        }
    })
    .unwrap_or(INTERNAL_ERROR)
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_file_read_sectors(
    handle: *const FileHandle,
    sector_index: u64,
    sector_size: u32,
    sector_count: u32,
    buffer: *mut u8,
    capacity: usize,
    bytes_read: *mut usize,
) -> i32 {
    if bytes_read.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { bytes_read.write(0) };
    let sector_size = if sector_size == 0 {
        DEFAULT_SECTOR_SIZE
    } else {
        sector_size
    };
    if handle.is_null() || buffer.is_null() || sector_count == 0 {
        return INVALID_ARGUMENT;
    }
    let Some(need) = (sector_count as u64).checked_mul(u64::from(sector_size)) else {
        return INVALID_RANGE;
    };
    let Ok(need) = usize::try_from(need) else {
        return INVALID_RANGE;
    };
    if need > MAX_READ_BYTES || capacity < need {
        return INVALID_RANGE;
    }
    catch_unwind(|| {
        let handle = unsafe { &*handle };
        let Ok(mut source) = handle.0.lock() else {
            return INTERNAL_ERROR;
        };
        let mut scratch = vec![0u8; need];
        match source.read_sectors(sector_index, sector_size, sector_count, &mut scratch) {
            Ok(receipt) => {
                unsafe {
                    if receipt.bytes_read != 0 {
                        ptr::copy_nonoverlapping(scratch.as_ptr(), buffer, receipt.bytes_read);
                    }
                    bytes_read.write(receipt.bytes_read);
                }
                OK
            }
            Err(error) => status(error),
        }
    })
    .unwrap_or(INTERNAL_ERROR)
}
