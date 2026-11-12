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
    ProcessVirtual,
    /// Byte-addressed storage view expressed in sector units by the caller.
    Storage,
}

/// Default logical sector size for storage explorers when the medium does not report one.
pub const DEFAULT_SECTOR_SIZE: u32 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadReceipt {
    pub address_space: AddressSpace,
    pub address: u64,
    pub captured_at: SystemTime,
    pub bytes_read: usize,
}

/// Owned bytes captured from one address range at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub address_space: AddressSpace,
    pub address: u64,
    pub captured_at: SystemTime,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteChange {
    pub address: u64,
    pub before: u8,
    pub after: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    InvalidRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareError {
    AddressSpaceMismatch,
    AddressMismatch,
    LengthMismatch,
}

impl MemorySnapshot {
    /// Builds a snapshot from already-read bytes. Length is capped by
    /// [`MAX_READ_BYTES`]. An empty buffer is allowed.
    pub fn capture(
        address_space: AddressSpace,
        address: u64,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, SnapshotError> {
        let bytes = bytes.into();
        if bytes.len() > MAX_READ_BYTES || address.checked_add(bytes.len() as u64).is_none() {
            return Err(SnapshotError::InvalidRange);
        }
        Ok(Self {
            address_space,
            address,
            captured_at: SystemTime::now(),
            bytes,
        })
    }
}

/// Compares two equal-length buffers that describe the same base address.
pub fn compare_bytes(
    base_address: u64,
    before: &[u8],
    after: &[u8],
) -> Result<Vec<ByteChange>, CompareError> {
    if before.len() != after.len() {
        return Err(CompareError::LengthMismatch);
    }
    if base_address.checked_add(before.len() as u64).is_none() {
        return Err(CompareError::AddressMismatch);
    }
    Ok(before
        .iter()
        .zip(after.iter())
        .enumerate()
        .filter_map(|(offset, (&left, &right))| {
            (left != right).then_some(ByteChange {
                address: base_address + offset as u64,
                before: left,
                after: right,
            })
        })
        .collect())
}

/// Diffs two snapshots that must cover the same space, base, and length.
pub fn compare_snapshots(
    before: &MemorySnapshot,
    after: &MemorySnapshot,
) -> Result<Vec<ByteChange>, CompareError> {
    if before.address_space != after.address_space {
        return Err(CompareError::AddressSpaceMismatch);
    }
    if before.address != after.address {
        return Err(CompareError::AddressMismatch);
    }
    compare_bytes(before.address, &before.bytes, &after.bytes)
}

#[derive(Debug)]
pub enum ReadError {
    InvalidRange,
    UnsupportedSource,
    Io(io::Error),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange => write!(f, "snapshot range exceeds the supported limit"),
        }
    }
}

impl std::error::Error for SnapshotError {}

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddressSpaceMismatch => write!(f, "snapshots use different address spaces"),
            Self::AddressMismatch => write!(f, "snapshots start at different addresses"),
            Self::LengthMismatch => write!(f, "snapshots cover different lengths"),
        }
    }
}

impl std::error::Error for CompareError {}

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
    use super::{
        compare_bytes, compare_snapshots, find_byte_offsets, AddressSpace, CompareError,
        MemorySnapshot, SnapshotError, MAX_READ_BYTES,
    };

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

    #[test]
    fn snapshot_compare_reports_changed_addresses() {
        let before = MemorySnapshot::capture(AddressSpace::File, 0x1000, vec![1, 2, 3]).unwrap();
        let after = MemorySnapshot::capture(AddressSpace::File, 0x1000, vec![1, 9, 3]).unwrap();
        let changes = compare_snapshots(&before, &after).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].address, 0x1001);
        assert_eq!(changes[0].before, 2);
        assert_eq!(changes[0].after, 9);
    }

    #[test]
    fn snapshot_compare_rejects_mismatched_metadata() {
        let file = MemorySnapshot::capture(AddressSpace::File, 0, vec![1]).unwrap();
        let process = MemorySnapshot::capture(AddressSpace::ProcessVirtual, 0, vec![1]).unwrap();
        assert_eq!(
            compare_snapshots(&file, &process),
            Err(CompareError::AddressSpaceMismatch)
        );

        let shifted = MemorySnapshot::capture(AddressSpace::File, 1, vec![1]).unwrap();
        assert_eq!(
            compare_snapshots(&file, &shifted),
            Err(CompareError::AddressMismatch)
        );

        let longer = MemorySnapshot::capture(AddressSpace::File, 0, vec![1, 2]).unwrap();
        assert_eq!(
            compare_snapshots(&file, &longer),
            Err(CompareError::LengthMismatch)
        );
    }

    #[test]
    fn snapshot_capture_rejects_oversized_and_overflowing_ranges() {
        assert_eq!(
            MemorySnapshot::capture(AddressSpace::File, 0, vec![0; MAX_READ_BYTES + 1]),
            Err(SnapshotError::InvalidRange)
        );
        assert_eq!(
            MemorySnapshot::capture(AddressSpace::File, u64::MAX, vec![0]),
            Err(SnapshotError::InvalidRange)
        );
    }

    #[test]
    fn compare_bytes_allows_identical_buffers() {
        assert!(compare_bytes(0x10, &[7, 8], &[7, 8]).unwrap().is_empty());
    }
}
