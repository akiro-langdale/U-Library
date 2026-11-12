//! Linux cache topology via `/sys/devices/system/cpu/*/cache/index*`.

use super::sysfs_parse::{parse_cache_kind, parse_cpu_list, parse_size_bytes};
use crate::{CacheError, CacheNode, CacheTopology};
use std::{fs, path::PathBuf};

fn read_trim(path: &std::path::Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

fn read_node(index_dir: PathBuf) -> Option<CacheNode> {
    let level: u32 = read_trim(&index_dir.join("level"))?.parse().ok()?;
    let kind = parse_cache_kind(&read_trim(&index_dir.join("type"))?);
    let size_bytes = read_trim(&index_dir.join("size")).and_then(|text| parse_size_bytes(&text));
    let line_size_bytes = read_trim(&index_dir.join("coherency_line_size"))
        .and_then(|text| text.parse::<u32>().ok())
        .filter(|value| *value > 0);
    let associativity_raw = read_trim(&index_dir.join("ways_of_associativity"))
        .and_then(|text| text.parse::<u32>().ok());
    let fully_associative = associativity_raw == Some(0);
    let associativity = associativity_raw.filter(|value| *value > 0);
    let shared_logical_processors =
        read_trim(&index_dir.join("shared_cpu_list")).map(|text| parse_cpu_list(&text))?;
    Some(CacheNode {
        level,
        kind,
        size_bytes,
        line_size_bytes,
        associativity,
        fully_associative,
        shared_logical_processors,
    })
}

fn node_key(node: &CacheNode) -> (u32, u8, Option<u64>, Vec<usize>) {
    (
        node.level,
        node.kind as u8,
        node.size_bytes,
        node.shared_logical_processors.clone(),
    )
}

pub(super) fn discover() -> Result<CacheTopology, CacheError> {
    let cpu_root = PathBuf::from("/sys/devices/system/cpu");
    let entries = match fs::read_dir(&cpu_root) {
        Ok(entries) => entries,
        Err(error) => {
            return Err(CacheError::Platform(format!(
                "failed to read {cpu_root:?}: {error}"
            )));
        }
    };

    let mut nodes = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("cpu") || !name[3..].chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let cache_root = entry.path().join("cache");
        let Ok(indexes) = fs::read_dir(cache_root) else {
            continue;
        };
        for index in indexes.flatten() {
            let index_name = index.file_name();
            let index_name = index_name.to_string_lossy();
            if !index_name.starts_with("index") {
                continue;
            }
            if let Some(node) = read_node(index.path()) {
                nodes.push(node);
            }
        }
    }

    if nodes.is_empty() {
        return Err(CacheError::Unavailable(
            "No cache topology entries were found under /sys/devices/system/cpu".into(),
        ));
    }

    nodes.sort_by_key(node_key);
    nodes.dedup_by_key(|node| node_key(node));
    nodes.sort_by_key(|node| (node.level, node.kind as u8));
    Ok(CacheTopology { nodes })
}
