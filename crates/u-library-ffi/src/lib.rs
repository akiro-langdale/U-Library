mod process;

use std::{ffi::c_char, panic::catch_unwind, ptr, slice, str, sync::Mutex};
use u_library_core::{ReadError, MAX_READ_BYTES};
use u_library_memory::FileSource;

const OK: i32 = 0;
const INVALID_ARGUMENT: i32 = 1;
const INVALID_RANGE: i32 = 2;
const UNSUPPORTED_SOURCE: i32 = 3;
const IO_ERROR: i32 = 4;
const INTERNAL_ERROR: i32 = 5;

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
