//! C ABI for CPU cache topology and hardware counters.

use super::{BUFFER_TOO_SMALL, INTERNAL_ERROR, INVALID_ARGUMENT, OK, UNSUPPORTED_SOURCE};
use std::panic::catch_unwind;
use u_library_cache::{discover, sample_counters, CacheError, CacheNode};

const PERMISSION_REQUIRED: i32 = 7;
const UNAVAILABLE: i32 = 8;
const IO_ERROR: i32 = 4;

pub const UL_CACHE_METRIC_UNAVAILABLE: u64 = u64::MAX;

const UL_CACHE_FLAG_FULLY_ASSOCIATIVE: u32 = 1u32 << 0;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CacheNodeC {
    pub level: u32,
    pub kind: u32,
    pub size_bytes: i64,
    pub line_size_bytes: u32,
    pub associativity: u32,
    pub flags: u32,
    pub shared_cpu_count: u32,
    pub reserved: u32,
}

fn status(error: CacheError) -> i32 {
    match error {
        CacheError::Unsupported(_) => UNSUPPORTED_SOURCE,
        CacheError::PermissionRequired(_) => PERMISSION_REQUIRED,
        CacheError::Unavailable(_) => UNAVAILABLE,
        CacheError::Platform(_) => IO_ERROR,
    }
}

fn write_node(node: &CacheNode) -> CacheNodeC {
    let mut flags = 0u32;
    if node.fully_associative {
        flags |= UL_CACHE_FLAG_FULLY_ASSOCIATIVE;
    }
    CacheNodeC {
        level: node.level,
        kind: node.kind as u32,
        size_bytes: node.size_bytes.map(i64::try_from).transpose().ok().flatten().unwrap_or(-1),
        line_size_bytes: node.line_size_bytes.unwrap_or(0),
        associativity: node.associativity.unwrap_or(0),
        flags,
        shared_cpu_count: u32::try_from(node.shared_logical_processors.len()).unwrap_or(u32::MAX),
        reserved: 0,
    }
}

#[no_mangle]
pub extern "C" fn ul_cache_abi_version() -> u32 {
    u_library_cache::cache_abi_version()
}

/// # Safety
/// See the pointer contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_cache_topology(
    buffer: *mut CacheNodeC,
    capacity: usize,
    count: *mut usize,
) -> i32 {
    if count.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe { count.write(0) };
    if capacity != 0 && buffer.is_null() {
        return INVALID_ARGUMENT;
    }
    if capacity > isize::MAX as usize / std::mem::size_of::<CacheNodeC>() {
        return INVALID_ARGUMENT;
    }
    catch_unwind(|| match discover() {
        Ok(topology) => {
            unsafe { count.write(topology.nodes.len()) };
            if capacity < topology.nodes.len() {
                return BUFFER_TOO_SMALL;
            }
            for (index, node) in topology.nodes.iter().enumerate() {
                unsafe {
                    buffer.add(index).write(write_node(node));
                }
            }
            OK
        }
        Err(error) => status(error),
    })
    .unwrap_or(INTERNAL_ERROR)
}

/// # Safety
/// All three output pointers are required.
#[no_mangle]
pub unsafe extern "C" fn ul_cache_counters(
    references: *mut u64,
    misses: *mut u64,
    window_nanos: *mut u64,
) -> i32 {
    if references.is_null() || misses.is_null() || window_nanos.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe {
        references.write(UL_CACHE_METRIC_UNAVAILABLE);
        misses.write(UL_CACHE_METRIC_UNAVAILABLE);
        window_nanos.write(UL_CACHE_METRIC_UNAVAILABLE);
    }
    catch_unwind(|| match sample_counters() {
        Ok(counters) => {
            unsafe {
                references.write(counters.references.unwrap_or(UL_CACHE_METRIC_UNAVAILABLE));
                misses.write(counters.misses.unwrap_or(UL_CACHE_METRIC_UNAVAILABLE));
                window_nanos.write(counters.window_nanos.unwrap_or(UL_CACHE_METRIC_UNAVAILABLE));
            }
            OK
        }
        Err(error) => status(error),
    })
    .unwrap_or(INTERNAL_ERROR)
}
