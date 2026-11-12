//! Shared `/proc/*/maps` line parser. Kept OS-independent for unit tests.

use super::{MemoryProtection, MemoryRegionKind, MemoryState, VirtualMemoryRegion};

/// One `/proc/*/maps` line.
/// Example: `7f8c4c000000-7f8c4c021000 r-xp 00000000 08:01 123 /usr/lib/libc.so.6`
pub(super) fn parse_maps_line(line: &str) -> Option<VirtualMemoryRegion> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let mut parts = line.split_whitespace();
    let range = parts.next()?;
    let perms = parts.next()?;
    let _offset = parts.next()?;
    let _dev = parts.next()?;
    let _inode = parts.next()?;
    let pathname = parts.collect::<Vec<_>>().join(" ");

    let (start, end) = range.split_once('-')?;
    let base_address = u64::from_str_radix(start, 16).ok()?;
    let end_address = u64::from_str_radix(end, 16).ok()?;
    let size_bytes = end_address.checked_sub(base_address)?;
    if size_bytes == 0 {
        return None;
    }

    let mut chars = perms.chars();
    let read = chars.next() == Some('r');
    let write = chars.next() == Some('w');
    let execute = chars.next() == Some('x');
    let share = chars.next();
    let protection = MemoryProtection {
        read,
        write,
        execute,
        guard: false,
    };
    Some(VirtualMemoryRegion {
        base_address,
        size_bytes,
        state: MemoryState::Committed,
        protection,
        kind: classify_kind(&pathname, share),
        readable: read,
    })
}

fn classify_kind(pathname: &str, share: Option<char>) -> MemoryRegionKind {
    if pathname.is_empty() {
        return if share == Some('s') {
            MemoryRegionKind::Mapped
        } else {
            MemoryRegionKind::Private
        };
    }
    if pathname.starts_with('[') {
        return MemoryRegionKind::Private;
    }
    if pathname.contains(".so") {
        return MemoryRegionKind::Image;
    }
    if share == Some('s') {
        MemoryRegionKind::Mapped
    } else {
        MemoryRegionKind::Image
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_anonymous_private_and_shared_library_lines() {
        let anon = parse_maps_line("7fff00000000-7fff00021000 rw-p 00000000 00:00 0").unwrap();
        assert_eq!(anon.base_address, 0x7fff_0000_0000);
        assert_eq!(anon.size_bytes, 0x21000);
        assert!(anon.protection.read && anon.protection.write && !anon.protection.execute);
        assert!(anon.readable);
        assert_eq!(anon.kind, MemoryRegionKind::Private);

        let lib =
            parse_maps_line("7f8c4c000000-7f8c4c021000 r-xp 00000000 08:01 123 /usr/lib/libc.so.6")
                .unwrap();
        assert!(lib.protection.read && lib.protection.execute && !lib.protection.write);
        assert_eq!(lib.kind, MemoryRegionKind::Image);

        let stack =
            parse_maps_line("7ffc00000000-7ffc00021000 rw-p 00000000 00:00 0 [stack]").unwrap();
        assert_eq!(stack.kind, MemoryRegionKind::Private);

        let shared_file =
            parse_maps_line("7f9000000000-7f9000001000 rw-s 00000000 08:01 99 /tmp/data.bin")
                .unwrap();
        assert_eq!(shared_file.kind, MemoryRegionKind::Mapped);
    }

    #[test]
    fn rejects_malformed_maps_lines() {
        assert!(parse_maps_line("").is_none());
        assert!(parse_maps_line("not-a-map-line").is_none());
        assert!(parse_maps_line("1000-1000 r--p 0 00:00 0").is_none());
    }
}
