//! Process virtual memory maps and bounded read-only samples.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(test, target_os = "linux"))]
mod maps_parse;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::ProcessMemory;
#[cfg(target_os = "macos")]
pub use macos::ProcessMemory;
#[cfg(windows)]
pub use windows::ProcessMemory;

use u_library_core::{AddressSpace, MemorySnapshot, SnapshotError};

/// Maximum length of one process memory read. Longer requests are capped.
pub const MAX_MEMORY_SAMPLE_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessMemoryError {
    InvalidRange(String),
    PermissionRequired(String),
    Unavailable(String),
    EnumerationLimit(String),
}

impl std::fmt::Display for ProcessMemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRange(message)
            | Self::PermissionRequired(message)
            | Self::Unavailable(message)
            | Self::EnumerationLimit(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ProcessMemoryError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryState {
    Free,
    Reserved,
    Committed,
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryProtection {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub guard: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionKind {
    Private,
    Image,
    Mapped,
    Free,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualMemoryRegion {
    pub base_address: u64,
    pub size_bytes: u64,
    pub state: MemoryState,
    pub protection: MemoryProtection,
    pub kind: MemoryRegionKind,
    pub readable: bool,
}

pub fn current_process_regions() -> Result<Vec<VirtualMemoryRegion>, ProcessMemoryError> {
    ProcessMemory::open(u64::from(std::process::id()))?.regions()
}

pub fn read_current_process(address: u64, length: usize) -> Result<Vec<u8>, ProcessMemoryError> {
    ProcessMemory::open(u64::from(std::process::id()))?.read(address, length)
}

fn snapshot_from_bytes(address: u64, bytes: Vec<u8>) -> Result<MemorySnapshot, ProcessMemoryError> {
    MemorySnapshot::capture(AddressSpace::ProcessVirtual, address, bytes).map_err(|error| {
        match error {
            SnapshotError::InvalidRange => ProcessMemoryError::InvalidRange(
                "Memory snapshot range exceeds the supported limit".into(),
            ),
        }
    })
}

fn validate_sample_range(address: u64, length: usize) -> Result<(usize, u64), ProcessMemoryError> {
    if length == 0 {
        return Err(ProcessMemoryError::InvalidRange(
            "Memory sample length must be greater than zero".into(),
        ));
    }
    let length = length.min(MAX_MEMORY_SAMPLE_BYTES);
    let end = address.checked_add(length as u64).ok_or_else(|| {
        ProcessMemoryError::InvalidRange("Memory sample range overflows the address space".into())
    })?;
    Ok((length, end))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_readable_span(
    regions: &[VirtualMemoryRegion],
    address: u64,
    end: u64,
) -> Result<(), ProcessMemoryError> {
    if regions.iter().any(|region| {
        region.readable
            && address >= region.base_address
            && end <= region.base_address.saturating_add(region.size_bytes)
    }) {
        Ok(())
    } else {
        Err(ProcessMemoryError::Unavailable(
            "Requested address is not in a readable mapped region".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_reads_enforce_sample_and_region_bounds() {
        static PAYLOAD: [u8; MAX_MEMORY_SAMPLE_BYTES] = [0xA5; MAX_MEMORY_SAMPLE_BYTES];

        let process = ProcessMemory::open(u64::from(std::process::id())).unwrap();
        let address = PAYLOAD.as_ptr() as usize as u64;
        assert_eq!(process.read(address, usize::MAX).unwrap(), PAYLOAD);
        assert!(matches!(
            process.read(address, 0),
            Err(ProcessMemoryError::InvalidRange(_))
        ));
        assert!(matches!(
            process.read(u64::MAX, 1),
            Err(ProcessMemoryError::InvalidRange(_))
        ));

        let region = process
            .regions()
            .unwrap()
            .into_iter()
            .find(|region| {
                region.readable
                    && (region.base_address..region.base_address + region.size_bytes)
                        .contains(&address)
            })
            .expect("readable payload region");
        let last_byte = region.base_address + region.size_bytes - 1;
        assert!(matches!(
            process.read(last_byte, 2),
            Err(ProcessMemoryError::Unavailable(_))
        ));

        let snapshot = process.snapshot(address, 4).unwrap();
        assert_eq!(snapshot.address_space, AddressSpace::ProcessVirtual);
        assert_eq!(snapshot.address, address);
        assert_eq!(snapshot.bytes, PAYLOAD[..4]);
    }
}
