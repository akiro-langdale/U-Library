//! Windows process memory via VirtualQueryEx / ReadProcessMemory.

use super::{
    snapshot_from_bytes, validate_sample_range, MemoryProtection, MemoryRegionKind, MemoryState,
    ProcessMemoryError, VirtualMemoryRegion,
};
use std::mem::{size_of, MaybeUninit};
use u_library_core::MemorySnapshot;
use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, HANDLE};
use windows::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_FREE, MEM_IMAGE, MEM_MAPPED,
    MEM_PRIVATE, MEM_RESERVE, PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
    PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_NOACCESS, PAGE_PROTECTION_FLAGS, PAGE_READONLY,
    PAGE_READWRITE, PAGE_WRITECOPY,
};
use windows::Win32::System::{
    Diagnostics::Debug::ReadProcessMemory,
    Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
};

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
        let (length, end) = validate_sample_range(address, length)?;
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
                self.0,
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
                self.0,
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

    pub fn snapshot(
        &self,
        address: u64,
        length: usize,
    ) -> Result<MemorySnapshot, ProcessMemoryError> {
        snapshot_from_bytes(address, self.read(address, length)?)
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
