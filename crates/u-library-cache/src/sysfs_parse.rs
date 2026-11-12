//! Parsers for Linux sysfs cache attributes. OS-independent for unit tests.

use crate::CacheKind;

pub(super) fn parse_size_bytes(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (number, scale) = text
        .char_indices()
        .find(|(_, ch)| !ch.is_ascii_digit())
        .map(|(index, _)| text.split_at(index))
        .unwrap_or((text, ""));
    let value: u64 = number.parse().ok()?;
    let multiplier = match scale.trim().to_ascii_uppercase().as_str() {
        "" | "B" => 1,
        "K" | "KB" => 1024,
        "M" | "MB" => 1024 * 1024,
        "G" | "GB" => 1024 * 1024 * 1024,
        _ => return None,
    };
    value.checked_mul(multiplier)
}

pub(super) fn parse_cache_kind(text: &str) -> CacheKind {
    match text.trim().to_ascii_lowercase().as_str() {
        "data" => CacheKind::Data,
        "instruction" => CacheKind::Instruction,
        "unified" => CacheKind::Unified,
        "trace" => CacheKind::Trace,
        _ => CacheKind::Unknown,
    }
}

/// Parses `shared_cpu_list` values such as `0-3,8,10-11`.
pub(super) fn parse_cpu_list(text: &str) -> Vec<usize> {
    let mut cpus = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start, end)) = part.split_once('-') {
            let Ok(start) = start.parse::<usize>() else {
                continue;
            };
            let Ok(end) = end.parse::<usize>() else {
                continue;
            };
            if end >= start {
                cpus.extend(start..=end);
            }
        } else if let Ok(cpu) = part.parse::<usize>() {
            cpus.push(cpu);
        }
    }
    cpus.sort_unstable();
    cpus.dedup();
    cpus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_size_kind_and_cpu_lists() {
        assert_eq!(parse_size_bytes("32K"), Some(32 * 1024));
        assert_eq!(parse_size_bytes("1M"), Some(1024 * 1024));
        assert_eq!(parse_size_bytes("64"), Some(64));
        assert_eq!(parse_cache_kind("Data"), CacheKind::Data);
        assert_eq!(parse_cache_kind("Unified"), CacheKind::Unified);
        assert_eq!(parse_cpu_list("0-3,8,10-11"), vec![0, 1, 2, 3, 8, 10, 11]);
        assert!(parse_size_bytes("").is_none());
    }

    #[test]
    fn fuzz_like_inputs_do_not_panic() {
        let mut state = 0x51AFC0DE_u32;
        for _ in 0..2_048 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let len = (state % 96) as usize;
            let bytes: Vec<u8> = (0..len)
                .map(|index| ((state >> (index % 20)) & 0xFF) as u8)
                .collect();
            let text = String::from_utf8_lossy(&bytes);
            let _ = parse_size_bytes(&text);
            let _ = parse_cache_kind(&text);
            let cpus = parse_cpu_list(&text);
            for pair in cpus.windows(2) {
                assert!(pair[0] <= pair[1]);
            }
        }
    }
}
