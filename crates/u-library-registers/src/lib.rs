//! CPU register capture for the current thread and consented other threads.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(all(windows, target_arch = "x86_64"))]
mod windows_x64;

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterArchitecture {
    X86_64 = 1,
    AArch64 = 2,
}

impl fmt::Display for RegisterArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86_64 => f.write_str("x86-64"),
            Self::AArch64 => f.write_str("AArch64"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterValue {
    pub name: String,
    pub bits: u64,
    pub width_bits: u8,
}

impl RegisterValue {
    pub fn new(name: impl Into<String>, bits: u64, width_bits: u8) -> Self {
        Self {
            name: name.into(),
            bits,
            width_bits,
        }
    }

    pub fn masked_bits(&self) -> u64 {
        if self.width_bits >= 64 {
            self.bits
        } else {
            let mask = (1u64 << self.width_bits).wrapping_sub(1);
            self.bits & mask
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterSnapshot {
    pub architecture: RegisterArchitecture,
    pub thread_id: u64,
    pub registers: Vec<RegisterValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterError {
    Unsupported(String),
    PermissionRequired(String),
    Unavailable(String),
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(message)
            | Self::PermissionRequired(message)
            | Self::Unavailable(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for RegisterError {}

/// Captures general-purpose registers of the calling thread when supported.
pub fn capture_current_thread() -> Result<RegisterSnapshot, RegisterError> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows_x64::capture_current()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        Err(RegisterError::Unsupported(
            "Current-thread register capture is implemented for Windows x86-64 in this version"
                .into(),
        ))
    }
}

/// Captures registers of another thread. Callers must obtain explicit consent first.
pub fn capture_thread(process_id: u64, thread_id: u64) -> Result<RegisterSnapshot, RegisterError> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows_x64::capture_thread(process_id, thread_id)
    }
    #[cfg(target_os = "linux")]
    {
        linux::capture_thread(process_id, thread_id)
    }
    #[cfg(not(any(all(windows, target_arch = "x86_64"), target_os = "linux")))]
    {
        let _ = (process_id, thread_id);
        Err(RegisterError::Unsupported(
            "Other-thread register capture is implemented for Windows x86-64 and Linux".into(),
        ))
    }
}

/// Returns 1 when at least one capture path exists on this build.
pub fn registers_abi_version() -> u32 {
    u32::from(cfg!(any(
        all(windows, target_arch = "x86_64"),
        target_os = "linux"
    )))
}
