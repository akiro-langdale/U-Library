use std::{fmt, io, time::SystemTime};

pub const MAX_READ_BYTES: usize = 1024 * 1024;

/// Returns byte offsets of exact matches, including overlapping matches.
/// An empty pattern yields no matches.
pub fn find_byte_offsets<'a>(
    bytes: &'a [u8],
    pattern: &'a [u8],
) -> impl Iterator<Item = usize> + 'a {
    bytes
        .windows(pattern.len().max(1))
        .enumerate()
        .filter_map(move |(offset, window)| (window == pattern).then_some(offset))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpace {
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadReceipt {
    pub address_space: AddressSpace,
    pub address: u64,
    pub captured_at: SystemTime,
    pub bytes_read: usize,
}

#[derive(Debug)]
pub enum ReadError {
    InvalidRange,
    UnsupportedSource,
    Io(io::Error),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange => write!(f, "read range exceeds the supported limit"),
            Self::UnsupportedSource => write!(f, "source is not a regular file"),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::find_byte_offsets;

    #[test]
    fn byte_search_reports_overlaps_and_includes_the_last_possible_offset() {
        assert_eq!(
            find_byte_offsets(&[0, 255, 255, 255], &[255, 255]).collect::<Vec<_>>(),
            [1, 2]
        );
    }

    #[test]
    fn byte_search_handles_empty_and_oversized_patterns() {
        assert_eq!(find_byte_offsets(&[0], &[]).next(), None);
        assert_eq!(find_byte_offsets(&[], &[0]).next(), None);
        assert_eq!(find_byte_offsets(&[0], &[0, 1]).next(), None);
        assert_eq!(find_byte_offsets(&[0], &[1]).next(), None);
    }
}
