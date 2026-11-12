//! Query-only GPU adapter inventory. Never invents VRAM contents or utilization.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuDevice {
    pub name: String,
    pub dedicated_bytes: Option<u64>,
    pub shared_bytes: Option<u64>,
    pub vendor_id: u32,
    pub device_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuError {
    Unsupported(String),
    Unavailable(String),
    PermissionRequired(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(message)
            | Self::Unavailable(message)
            | Self::PermissionRequired(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for GpuError {}

/// Returns 1 when adapter enumeration is built for this target.
pub fn gpu_abi_version() -> u32 {
    u32::from(cfg!(windows))
}

/// Lists visible GPU adapters. Dedicated/shared bytes stay absent when unknown.
pub fn list_devices() -> Result<Vec<GpuDevice>, GpuError> {
    #[cfg(windows)]
    {
        windows_list()
    }
    #[cfg(not(windows))]
    {
        Err(GpuError::Unsupported(
            "GPU adapter enumeration is implemented for Windows DXGI in this version".into(),
        ))
    }
}

#[cfg(windows)]
fn windows_list() -> Result<Vec<GpuDevice>, GpuError> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

    // SAFETY: CreateDXGIFactory1 returns a COM factory; Release runs via Drop of the Rc-like iface.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(|error| {
        GpuError::Unavailable(format!("CreateDXGIFactory1 failed: {error}"))
    })?;

    let mut devices = Vec::new();
    let mut index = 0u32;
    loop {
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(_) => break,
        };
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
            index += 1;
            continue;
        };
        let name = String::from_utf16_lossy(
            &desc
                .Description
                .iter()
                .copied()
                .take_while(|unit| *unit != 0)
                .collect::<Vec<_>>(),
        );
        let dedicated = (desc.DedicatedVideoMemory != 0).then_some(desc.DedicatedVideoMemory as u64);
        let shared = (desc.SharedSystemMemory != 0).then_some(desc.SharedSystemMemory as u64);
        devices.push(GpuDevice {
            name,
            dedicated_bytes: dedicated,
            shared_bytes: shared,
            vendor_id: desc.VendorId,
            device_id: desc.DeviceId,
        });
        index = index.saturating_add(1);
        if index > 256 {
            break;
        }
    }
    if devices.is_empty() {
        return Err(GpuError::Unavailable(
            "DXGI reported no GPU adapters".into(),
        ));
    }
    Ok(devices)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_lists_at_least_one_adapter() {
        let devices = list_devices().unwrap();
        assert!(!devices.is_empty());
        assert!(!devices[0].name.is_empty());
    }
}
