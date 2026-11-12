//! CPU cache topology discovery and optional hardware counters.

#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

mod counters;

// Available on all hosts so Linux sysfs parsers are unit-tested under Windows CI.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod sysfs_parse;

use std::fmt;

/// Contents held by a cache level, as reported by the OS/firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CacheKind {
    Data = 1,
    Instruction = 2,
    Unified = 3,
    Trace = 4,
    Unknown = 5,
}

impl fmt::Display for CacheKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Data => "DATA",
            Self::Instruction => "INSTRUCTION",
            Self::Unified => "UNIFIED",
            Self::Trace => "TRACE",
            Self::Unknown => "UNKNOWN",
        })
    }
}

/// One OS-reported cache node in the hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheNode {
    pub level: u32,
    pub kind: CacheKind,
    pub size_bytes: Option<u64>,
    pub line_size_bytes: Option<u32>,
    pub associativity: Option<u32>,
    pub fully_associative: bool,
    pub shared_logical_processors: Vec<usize>,
}

/// Complete topology snapshot for the host as visible to this process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheTopology {
    pub nodes: Vec<CacheNode>,
}

/// Hardware cache reference/miss counts over a short sample window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheCounters {
    pub references: Option<u64>,
    pub misses: Option<u64>,
    pub window_nanos: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheError {
    Unsupported(String),
    Unavailable(String),
    PermissionRequired(String),
    Platform(String),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(message)
            | Self::Unavailable(message)
            | Self::PermissionRequired(message)
            | Self::Platform(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CacheError {}

/// Discovers cache hierarchy metadata (sizes, levels, sharing). Never cache contents.
pub fn discover() -> Result<CacheTopology, CacheError> {
    #[cfg(windows)]
    {
        windows::discover()
    }
    #[cfg(target_os = "linux")]
    {
        linux::discover()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err(CacheError::Unsupported(
            "Cache topology discovery is implemented for Windows and Linux in this version".into(),
        ))
    }
}

/// Samples hardware cache references/misses when the OS exposes PMC/perf access.
pub fn sample_counters() -> Result<CacheCounters, CacheError> {
    counters::sample()
}

/// Returns 1 when topology discovery is built for this target, otherwise 0.
pub fn cache_abi_version() -> u32 {
    u32::from(cfg!(any(windows, target_os = "linux")))
}
