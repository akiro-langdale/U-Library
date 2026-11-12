//! C ABI for GPU adapter inventory.

use super::{BUFFER_TOO_SMALL, INTERNAL_ERROR, INVALID_ARGUMENT, OK, UNSUPPORTED_SOURCE};
use std::panic::catch_unwind;
use u_library_gpu::{list_devices, GpuDevice, GpuError};

const PERMISSION_REQUIRED: i32 = 7;
const UNAVAILABLE: i32 = 8;
const NAME_LEN: usize = 128;

#[repr(C)]
pub struct GpuDeviceAbi {
    name: [u8; NAME_LEN],
    dedicated_bytes: u64,
    shared_bytes: u64,
    vendor_id: u32,
    device_id: u32,
    reserved: [u8; 8],
}

fn status(error: GpuError) -> i32 {
    match error {
        GpuError::Unsupported(_) => UNSUPPORTED_SOURCE,
        GpuError::Unavailable(_) => UNAVAILABLE,
        GpuError::PermissionRequired(_) => PERMISSION_REQUIRED,
    }
}

fn encode(device: GpuDevice) -> GpuDeviceAbi {
    let mut name = [0u8; NAME_LEN];
    let bytes = device.name.as_bytes();
    let copy = bytes.len().min(NAME_LEN.saturating_sub(1));
    name[..copy].copy_from_slice(&bytes[..copy]);
    GpuDeviceAbi {
        name,
        dedicated_bytes: device.dedicated_bytes.unwrap_or(u64::MAX),
        shared_bytes: device.shared_bytes.unwrap_or(u64::MAX),
        vendor_id: device.vendor_id,
        device_id: device.device_id,
        reserved: [0; 8],
    }
}

#[no_mangle]
pub extern "C" fn ul_gpu_abi_version() -> u32 {
    u_library_gpu::gpu_abi_version()
}

/// # Safety
/// See the pointer and ownership contract in u_library.h.
#[no_mangle]
pub unsafe extern "C" fn ul_gpu_list(
    buffer: *mut GpuDeviceAbi,
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
    if capacity > isize::MAX as usize / std::mem::size_of::<GpuDeviceAbi>() {
        return INVALID_ARGUMENT;
    }
    catch_unwind(|| match list_devices() {
        Ok(devices) => {
            unsafe { count.write(devices.len()) };
            if capacity < devices.len() {
                return BUFFER_TOO_SMALL;
            }
            for (index, device) in devices.into_iter().enumerate() {
                unsafe { buffer.add(index).write(encode(device)) };
            }
            OK
        }
        Err(error) => status(error),
    })
    .unwrap_or(INTERNAL_ERROR)
}
