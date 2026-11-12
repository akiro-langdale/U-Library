//! Windows process memory maps and bounded read-only samples.

use std::mem::{size_of, MaybeUninit};
use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, HANDLE};
use windows::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_FREE, MEM_IMAGE, MEM_MAPPED,
    MEM_PRIVATE, MEM_RESERVE, PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
    PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_NOACCESS, PAGE_PROTECTION_FLAGS, PAGE_READONLY,
    PAGE_READWRITE, PAGE_WRITECOPY,
};
use windows::Win32::System::{
    Diagnostics::Debug::ReadProcessMemory,
    Threading::{GetCurrentProcess, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
};

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
/// Owns a read-only process handle so subsequent reads stay on the same target.
pub struct ProcessMemory(HANDLE);

impl ProcessMemory {
    pub fn open(pid: u64) -> Result<Self, ProcessMemoryError> {
        let Ok(pid) = u32::try_from(pid) else {
            return Err(ProcessMemoryError::Unavailable(
                "PID exceeds the Windows PID range".into(),
            ));
        };
        // SAFETY: the handle is non-inheritable and requests query and read access.
        match unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) } {
            Ok(handle) => Ok(Self(handle)),
            Err(error) if error.code() == E_ACCESSDENIED => Err(
                ProcessMemoryError::PermissionRequired(format!("OpenProcess({pid}): {error}")),
            ),
            Err(error) => Err(ProcessMemoryError::Unavailable(format!(
                "OpenProcess({pid}): {error}"
            ))),
        }
    }

    pub fn regions(&self) -> Result<Vec<VirtualMemoryRegion>, ProcessMemoryError> {
        enumerate_process_regions(self.0)
    }

    pub fn read(&self, address: u64, length: usize) -> Result<Vec<u8>, ProcessMemoryError> {
        read_process_bytes(self.0, address, length)
    }
}

impl Drop for ProcessMemory {
    fn drop(&mut self) {
        // SAFETY: the session owns this handle.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn protection_from(flags: PAGE_PROTECTION_FLAGS) -> MemoryProtection {
    let bits = flags.0;
    let guard = bits & PAGE_GUARD.0 != 0;
    let access = bits & !PAGE_GUARD.0;
    let (read, write, execute) = if access == PAGE_NOACCESS.0 {
        (false, false, false)
    } else if access == PAGE_READONLY.0 {
        (true, false, false)
    } else if access == PAGE_READWRITE.0 || access == PAGE_WRITECOPY.0 {
        (true, true, false)
    } else if access == PAGE_EXECUTE.0 {
        (false, false, true)
    } else if access == PAGE_EXECUTE_READ.0 {
        (true, false, true)
    } else if access == PAGE_EXECUTE_READWRITE.0 || access == PAGE_EXECUTE_WRITECOPY.0 {
        (true, true, true)
    } else {
        // Unknown combinations stay conservative: no implied readability.
        (false, false, false)
    };
    MemoryProtection {
        read,
        write,
        execute,
        guard,
    }
}

fn state_from(state: windows::Win32::System::Memory::VIRTUAL_ALLOCATION_TYPE) -> MemoryState {
    if state == MEM_COMMIT {
        MemoryState::Committed
    } else if state == MEM_RESERVE {
        MemoryState::Reserved
    } else if state == MEM_FREE {
        MemoryState::Free
    } else {
        MemoryState::Unknown
    }
}

fn kind_from(
    region_type: windows::Win32::System::Memory::PAGE_TYPE,
    state: MemoryState,
) -> MemoryRegionKind {
    if state == MemoryState::Free {
        return MemoryRegionKind::Free;
    }
    if region_type == MEM_IMAGE {
        MemoryRegionKind::Image
    } else if region_type == MEM_MAPPED {
        MemoryRegionKind::Mapped
    } else if region_type == MEM_PRIVATE {
        MemoryRegionKind::Private
    } else {
        MemoryRegionKind::Unknown
    }
}

fn region_from(info: &MEMORY_BASIC_INFORMATION) -> VirtualMemoryRegion {
    let state = state_from(info.State);
    let protection = if state == MemoryState::Committed {
        protection_from(info.Protect)
    } else {
        MemoryProtection::default()
    };
    let readable = state == MemoryState::Committed && protection.read && !protection.guard;
    VirtualMemoryRegion {
        base_address: info.BaseAddress as usize as u64,
        size_bytes: info.RegionSize as u64,
        state,
        protection,
        kind: kind_from(info.Type, state),
        readable,
    }
}

pub fn current_process_regions() -> Result<Vec<VirtualMemoryRegion>, ProcessMemoryError> {
    // SAFETY: GetCurrentProcess returns a borrowed pseudo-handle.
    enumerate_process_regions(unsafe { GetCurrentProcess() })
}

fn enumerate_process_regions(
    process: HANDLE,
) -> Result<Vec<VirtualMemoryRegion>, ProcessMemoryError> {
    let mut regions = Vec::new();
    let mut address = 0usize;
    loop {
        let mut info = MaybeUninit::<MEMORY_BASIC_INFORMATION>::uninit();
        // SAFETY: VirtualQueryEx writes a complete MEMORY_BASIC_INFORMATION on success.
        let written = unsafe {
            VirtualQueryEx(
                process,
                Some(address as *const _),
                info.as_mut_ptr(),
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if written != size_of::<MEMORY_BASIC_INFORMATION>() {
            break;
        }
        // SAFETY: VirtualQueryEx reported a complete structure.
        let info = unsafe { info.assume_init() };
        let region = region_from(&info);
        let next = match (region.base_address as usize).checked_add(region.size_bytes as usize) {
            Some(value) if value > address => value,
            _ => break,
        };
        regions.push(region);
        address = next;
        if regions.len() > 100_000 {
            return Err(ProcessMemoryError::EnumerationLimit(
                "Virtual memory enumeration exceeded the safety limit".into(),
            ));
        }
    }
    if regions.is_empty() {
        return Err(ProcessMemoryError::Unavailable(
            "No virtual memory regions were reported".into(),
        ));
    }
    Ok(regions)
}

pub fn read_current_process(address: u64, length: usize) -> Result<Vec<u8>, ProcessMemoryError> {
    // SAFETY: GetCurrentProcess returns a borrowed pseudo-handle.
    read_process_bytes(unsafe { GetCurrentProcess() }, address, length)
}

fn read_process_bytes(
    process: HANDLE,
    address: u64,
    length: usize,
) -> Result<Vec<u8>, ProcessMemoryError> {
    if length == 0 {
        return Err(ProcessMemoryError::InvalidRange(
            "Memory sample length must be greater than zero".into(),
        ));
    }
    let length = length.min(MAX_MEMORY_SAMPLE_BYTES);
    let end = address.checked_add(length as u64).ok_or_else(|| {
        ProcessMemoryError::InvalidRange("Memory sample range overflows the address space".into())
    })?;
    if usize::try_from(end - 1).is_err() {
        return Err(ProcessMemoryError::InvalidRange(
            "Memory sample range exceeds the native pointer width".into(),
        ));
    }
    let pointer = address as usize as *const _;
    let mut info = MaybeUninit::<MEMORY_BASIC_INFORMATION>::uninit();
    // SAFETY: the output buffer can hold one MEMORY_BASIC_INFORMATION.
    let written = unsafe {
        VirtualQueryEx(
            process,
            Some(pointer),
            info.as_mut_ptr(),
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if written != size_of::<MEMORY_BASIC_INFORMATION>() {
        return Err(ProcessMemoryError::Unavailable(
            "VirtualQuery could not resolve the requested address".into(),
        ));
    }
    // SAFETY: VirtualQueryEx reported that it initialized the entire structure.
    let region = region_from(&unsafe { info.assume_init() });
    if !region.readable {
        return Err(ProcessMemoryError::Unavailable(
            "Requested address is not in a readable committed region".into(),
        ));
    }
    let region_end = region.base_address.saturating_add(region.size_bytes);
    if address < region.base_address || address >= region_end {
        return Err(ProcessMemoryError::Unavailable(
            "Requested address is outside the resolved virtual region".into(),
        ));
    }
    if end > region_end {
        return Err(ProcessMemoryError::Unavailable(
            "Requested sample crosses into a non-readable or different region".into(),
        ));
    }

    let mut bytes = vec![0; length];
    let mut copied = 0;
    // SAFETY: Windows validates the source range and bytes has length writable bytes.
    let result = unsafe {
        ReadProcessMemory(
            process,
            pointer,
            bytes.as_mut_ptr().cast(),
            length,
            Some(&mut copied),
        )
    };
    if result.is_err() || copied != length {
        return Err(ProcessMemoryError::Unavailable(
            "Windows could not read the full range; the mapping may have changed".into(),
        ));
    }
    Ok(bytes)
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
    }
}
