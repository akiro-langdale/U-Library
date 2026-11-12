//! Windows cache topology via GetLogicalProcessorInformationEx(RelationCache).

use crate::{CacheError, CacheKind, CacheNode, CacheTopology};
use std::mem::size_of;
use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows::Win32::System::SystemInformation::{
    CacheData, CacheInstruction, CacheTrace, CacheUnified, GetLogicalProcessorInformationEx,
    RelationCache, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
};

fn kind_from(value: windows::Win32::System::SystemInformation::PROCESSOR_CACHE_TYPE) -> CacheKind {
    if value == CacheData {
        CacheKind::Data
    } else if value == CacheInstruction {
        CacheKind::Instruction
    } else if value == CacheUnified {
        CacheKind::Unified
    } else if value == CacheTrace {
        CacheKind::Trace
    } else {
        CacheKind::Unknown
    }
}

fn processors_from_mask(mask: usize) -> Vec<usize> {
    (0..usize::BITS as usize)
        .filter(|bit| mask & (1usize << bit) != 0)
        .collect()
}

pub(super) fn discover() -> Result<CacheTopology, CacheError> {
    let mut length = 0u32;
    match unsafe { GetLogicalProcessorInformationEx(RelationCache, None, &mut length) } {
        Ok(()) => {
            return Err(CacheError::Unavailable(
                "Cache topology buffer size was not reported".into(),
            ));
        }
        Err(error) if error.code() == ERROR_INSUFFICIENT_BUFFER.into() && length > 0 => {}
        Err(error) => {
            return Err(CacheError::Platform(format!(
                "GetLogicalProcessorInformationEx size probe failed: {error}"
            )));
        }
    }

    let mut buffer = vec![0u8; length as usize];
    if let Err(error) = unsafe {
        GetLogicalProcessorInformationEx(
            RelationCache,
            Some(buffer.as_mut_ptr().cast()),
            &mut length,
        )
    } {
        return Err(CacheError::Platform(format!(
            "GetLogicalProcessorInformationEx failed: {error}"
        )));
    }

    const HEADER_BYTES: usize = size_of::<u32>() * 2;
    let mut nodes = Vec::new();
    let mut offset = 0usize;
    let usable = length as usize;
    while offset < usable {
        if offset + HEADER_BYTES > buffer.len() || offset + HEADER_BYTES > usable {
            return Err(CacheError::Platform(
                "Truncated cache topology header".into(),
            ));
        }
        let info_ptr = unsafe {
            buffer.as_ptr().add(offset) as *const SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX
        };
        let size = unsafe { (*info_ptr).Size as usize };
        if size < HEADER_BYTES || offset + size > buffer.len() || offset + size > usable {
            return Err(CacheError::Platform(
                "Malformed cache topology record size".into(),
            ));
        }
        let relationship = unsafe { (*info_ptr).Relationship };
        if relationship == RelationCache {
            let cache = unsafe { (*info_ptr).Anonymous.Cache };
            let fully_associative = cache.Associativity == 0xff;
            let associativity = if fully_associative || cache.Associativity == 0 {
                None
            } else {
                Some(u32::from(cache.Associativity))
            };
            let size_bytes = if cache.CacheSize == 0 {
                None
            } else {
                Some(u64::from(cache.CacheSize))
            };
            let line_size_bytes = if cache.LineSize == 0 {
                None
            } else {
                Some(u32::from(cache.LineSize))
            };
            let mask = unsafe { cache.Anonymous.GroupMask.Mask };
            nodes.push(CacheNode {
                level: u32::from(cache.Level),
                kind: kind_from(cache.Type),
                size_bytes,
                line_size_bytes,
                associativity,
                fully_associative,
                shared_logical_processors: processors_from_mask(mask),
            });
        }
        offset += size;
    }

    if nodes.is_empty() {
        return Err(CacheError::Unavailable(
            "No cache topology entries were reported".into(),
        ));
    }
    nodes.sort_by_key(|node| (node.level, node.kind as u8));
    Ok(CacheTopology { nodes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_reports_at_least_one_cache_level_on_windows() {
        let topology = discover().expect("expected cache topology");
        assert!(!topology.nodes.is_empty());
        assert!(topology.nodes.iter().any(|node| node.level >= 1));
    }
}
