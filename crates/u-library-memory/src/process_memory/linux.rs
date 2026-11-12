//! Linux process memory via `/proc/<pid>/maps` and `/proc/<pid>/mem`.

use super::{
    maps_parse::parse_maps_line, require_readable_span, snapshot_from_bytes, validate_sample_range,
    ProcessMemoryError, VirtualMemoryRegion,
};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
    sync::Mutex,
};
use u_library_core::MemorySnapshot;

/// Owns a read-only `/proc/<pid>/mem` handle for a fixed target process.
pub struct ProcessMemory {
    maps_path: PathBuf,
    mem: Mutex<File>,
}

impl ProcessMemory {
    pub fn open(pid: u64) -> Result<Self, ProcessMemoryError> {
        let (maps_path, mem_path) = if pid == u64::from(std::process::id()) {
            (
                PathBuf::from("/proc/self/maps"),
                PathBuf::from("/proc/self/mem"),
            )
        } else {
            (
                PathBuf::from(format!("/proc/{pid}/maps")),
                PathBuf::from(format!("/proc/{pid}/mem")),
            )
        };
        match std::fs::read_to_string(&maps_path) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                return Err(ProcessMemoryError::PermissionRequired(format!(
                    "open({}): {error}",
                    maps_path.display()
                )));
            }
            Err(error) => {
                return Err(ProcessMemoryError::Unavailable(format!(
                    "open({}): {error}",
                    maps_path.display()
                )));
            }
        }
        match File::open(&mem_path) {
            Ok(mem) => Ok(Self {
                maps_path,
                mem: Mutex::new(mem),
            }),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                Err(ProcessMemoryError::PermissionRequired(format!(
                    "open({}): {error}",
                    mem_path.display()
                )))
            }
            Err(error) => Err(ProcessMemoryError::Unavailable(format!(
                "open({}): {error}",
                mem_path.display()
            ))),
        }
    }

    pub fn regions(&self) -> Result<Vec<VirtualMemoryRegion>, ProcessMemoryError> {
        let text = match std::fs::read_to_string(&self.maps_path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                return Err(ProcessMemoryError::PermissionRequired(format!(
                    "read({}): {error}",
                    self.maps_path.display()
                )));
            }
            Err(error) => {
                return Err(ProcessMemoryError::Unavailable(format!(
                    "read({}): {error}",
                    self.maps_path.display()
                )));
            }
        };
        let mut regions = Vec::new();
        for line in text.lines() {
            if let Some(region) = parse_maps_line(line) {
                regions.push(region);
            }
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

    pub fn read(&self, address: u64, length: usize) -> Result<Vec<u8>, ProcessMemoryError> {
        let (length, end) = validate_sample_range(address, length)?;
        require_readable_span(&self.regions()?, address, end)?;
        let Ok(mut mem) = self.mem.lock() else {
            return Err(ProcessMemoryError::Unavailable(
                "process memory session lock is poisoned".into(),
            ));
        };
        if let Err(error) = mem.seek(SeekFrom::Start(address)) {
            return Err(ProcessMemoryError::Unavailable(format!(
                "cannot seek /proc/*/mem to 0x{address:X}: {error}"
            )));
        }
        let mut bytes = vec![0; length];
        match mem.read_exact(&mut bytes) {
            Ok(()) => Ok(bytes),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                Err(ProcessMemoryError::PermissionRequired(format!(
                    "failed to read {length} bytes from /proc/*/mem: {error}"
                )))
            }
            Err(error) => Err(ProcessMemoryError::Unavailable(format!(
                "failed to read {length} bytes from /proc/*/mem: {error}"
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
