//! macOS process memory via Mach VM.

use super::{
    require_readable_span, snapshot_from_bytes, validate_sample_range, MemoryProtection,
    MemoryRegionKind, MemoryState, ProcessMemoryError, VirtualMemoryRegion,
};
use std::mem;
use u_library_core::MemorySnapshot;

/// Owns a Mach task port used for region queries and read-only samples.
pub struct ProcessMemory {
    task: mach2::port::mach_port_t,
    owns_task: bool,
}

impl ProcessMemory {
    pub fn open(pid: u64) -> Result<Self, ProcessMemoryError> {
        let Ok(pid) = i32::try_from(pid) else {
            return Err(ProcessMemoryError::Unavailable(
                "PID exceeds the macOS PID range".into(),
            ));
        };
        if pid as u32 == std::process::id() {
            // SAFETY: mach_task_self returns a borrowed port for this task.
            return Ok(Self {
                task: unsafe { mach2::traps::mach_task_self() },
                owns_task: false,
            });
        }
        let mut task = 0;
        // SAFETY: task receives a new send right on success; caller owns it.
        let status =
            unsafe { mach2::traps::task_for_pid(mach2::traps::mach_task_self(), pid, &mut task) };
        if status != mach2::kern_return::KERN_SUCCESS {
            return Err(ProcessMemoryError::PermissionRequired(format!(
                "task_for_pid({pid}) failed with Mach status {status}"
            )));
        }
        Ok(Self {
            task,
            owns_task: true,
        })
    }

    pub fn regions(&self) -> Result<Vec<VirtualMemoryRegion>, ProcessMemoryError> {
        let mut address: mach2::vm_types::mach_vm_address_t = 1;
        let mut regions = Vec::new();
        loop {
            let mut size: mach2::vm_types::mach_vm_size_t = 0;
            let mut info: mach2::vm_region::vm_region_basic_info_64 = unsafe { mem::zeroed() };
            let mut count = mach2::vm_region::vm_region_basic_info_64::count();
            let mut object_name: mach2::port::mach_port_t = 0;
            // SAFETY: Mach fills info/size on success; object_name is unused.
            let status = unsafe {
                mach2::vm::mach_vm_region(
                    self.task,
                    &mut address,
                    &mut size,
                    mach2::vm_region::VM_REGION_BASIC_INFO_64,
                    &mut info as *mut _ as *mut i32,
                    &mut count,
                    &mut object_name,
                )
            };
            if status != mach2::kern_return::KERN_SUCCESS || size == 0 {
                break;
            }
            let read = (info.protection & mach2::vm_prot::VM_PROT_READ) != 0;
            let write = (info.protection & mach2::vm_prot::VM_PROT_WRITE) != 0;
            let execute = (info.protection & mach2::vm_prot::VM_PROT_EXECUTE) != 0;
            regions.push(VirtualMemoryRegion {
                base_address: address,
                size_bytes: size,
                state: MemoryState::Committed,
                protection: MemoryProtection {
                    read,
                    write,
                    execute,
                    guard: false,
                },
                kind: if info.shared != 0 {
                    MemoryRegionKind::Mapped
                } else {
                    MemoryRegionKind::Private
                },
                readable: read,
            });
            let Some(next) = address.checked_add(size) else {
                break;
            };
            address = next;
            if address == 0 {
                break;
            }
            if regions.len() > 100_000 {
                return Err(ProcessMemoryError::EnumerationLimit(
                    "Virtual memory enumeration exceeded the safety limit".into(),
                ));
            }
        }
        if regions.is_empty() {
            return Err(ProcessMemoryError::Unavailable(
                "mach_vm_region returned no regions for the target task".into(),
            ));
        }
        Ok(regions)
    }

    pub fn read(&self, address: u64, length: usize) -> Result<Vec<u8>, ProcessMemoryError> {
        let (length, end) = validate_sample_range(address, length)?;
        require_readable_span(&self.regions()?, address, end)?;
        let mut bytes = vec![0; length];
        let size = length as mach2::vm_types::mach_vm_size_t;
        let mut copied = 0;
        // SAFETY: destination holds `size` writable bytes; Mach rechecks the mapping.
        let status = unsafe {
            mach2::vm::mach_vm_read_overwrite(
                self.task,
                address,
                size,
                bytes.as_mut_ptr() as usize as mach2::vm_types::mach_vm_address_t,
                &mut copied,
            )
        };
        match status {
            mach2::kern_return::KERN_SUCCESS if copied == size => Ok(bytes),
            mach2::kern_return::KERN_SUCCESS => Err(ProcessMemoryError::Unavailable(format!(
                "mach_vm_read_overwrite returned {copied} of {size} bytes"
            ))),
            mach2::kern_return::KERN_PROTECTION_FAILURE => {
                Err(ProcessMemoryError::PermissionRequired(
                    "Mach denied read access to the requested memory range".into(),
                ))
            }
            _ => Err(ProcessMemoryError::Unavailable(format!(
                "mach_vm_read_overwrite failed at 0x{address:X} (Mach status {status})"
            ))),
        }
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
        if self.owns_task {
            // SAFETY: owns_task means this send right was returned by task_for_pid.
            let _ = unsafe {
                mach2::mach_port::mach_port_deallocate(mach2::traps::mach_task_self(), self.task)
            };
        }
    }
}
