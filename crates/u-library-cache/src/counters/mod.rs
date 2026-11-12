//! Hardware cache counter sampling. Never invents hits/misses.

#[cfg(target_os = "linux")]
mod linux;

use crate::{CacheCounters, CacheError};

pub(super) fn sample() -> Result<CacheCounters, CacheError> {
    #[cfg(target_os = "linux")]
    {
        linux::sample()
    }
    #[cfg(windows)]
    {
        Err(CacheError::Unavailable(
            "Windows user-mode PMC for cache references/misses is not exposed without a driver; values are not simulated"
                .into(),
        ))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err(CacheError::Unsupported(
            "Hardware cache counters are implemented for Linux perf_event_open in this version"
                .into(),
        ))
    }
}
