/// Process virtual memory maps and bounded read-only samples.
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub mod process_memory;

use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    time::SystemTime,
};
use u_library_core::{
    AddressSpace, MemorySnapshot, ReadError, ReadReceipt, SnapshotError, MAX_READ_BYTES,
};

/// Translates a sector index into a byte offset.
pub fn sector_byte_offset(sector_index: u64, sector_size: u32) -> Result<u64, ReadError> {
    if sector_size == 0 {
        return Err(ReadError::InvalidRange);
    }
    sector_index
        .checked_mul(u64::from(sector_size))
        .ok_or(ReadError::InvalidRange)
}

/// Reads bytes at offsets in an open regular file.
pub struct FileSource {
    file: File,
}

impl FileSource {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ReadError> {
        let path = path.as_ref();
        if !fs::metadata(path)?.is_file() {
            return Err(ReadError::UnsupportedSource);
        }
        let file = File::open(path)?;
        if !file.metadata()?.is_file() {
            return Err(ReadError::UnsupportedSource);
        }
        Ok(Self { file })
    }

    /// On success only the first `bytes_read` bytes are initialized by this operation.
    /// EOF is a successful short read. On error, discard the buffer contents.
    pub fn read_at(&mut self, address: u64, buffer: &mut [u8]) -> Result<ReadReceipt, ReadError> {
        if buffer.len() > MAX_READ_BYTES || address.checked_add(buffer.len() as u64).is_none() {
            return Err(ReadError::InvalidRange);
        }
        let mut bytes_read = 0;
        if !buffer.is_empty() && address < self.file.metadata()?.len() {
            self.file.seek(SeekFrom::Start(address))?;
            while bytes_read < buffer.len() {
                match self.file.read(&mut buffer[bytes_read..]) {
                    Ok(0) => break,
                    Ok(count) => bytes_read += count,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Ok(ReadReceipt {
            address_space: AddressSpace::File,
            address,
            captured_at: SystemTime::now(),
            bytes_read,
        })
    }

    /// Reads a bounded range and keeps the returned bytes as a snapshot.
    pub fn snapshot_at(
        &mut self,
        address: u64,
        length: usize,
    ) -> Result<MemorySnapshot, ReadError> {
        if length > MAX_READ_BYTES || address.checked_add(length as u64).is_none() {
            return Err(ReadError::InvalidRange);
        }
        let mut bytes = vec![0; length];
        let receipt = self.read_at(address, &mut bytes)?;
        bytes.truncate(receipt.bytes_read);
        MemorySnapshot::capture(AddressSpace::File, address, bytes).map_err(|error| match error {
            SnapshotError::InvalidRange => ReadError::InvalidRange,
        })
    }

    /// Returns the current file length in bytes.
    pub fn size_bytes(&self) -> Result<u64, ReadError> {
        Ok(self.file.metadata()?.len())
    }

    /// Reads whole sectors starting at `sector_index`.
    /// `sector_size` defaults to [`DEFAULT_SECTOR_SIZE`] when zero is passed by C callers
    /// after translation; Rust callers must pass a non-zero size.
    pub fn read_sectors(
        &mut self,
        sector_index: u64,
        sector_size: u32,
        sector_count: u32,
        buffer: &mut [u8],
    ) -> Result<ReadReceipt, ReadError> {
        if sector_size == 0 || sector_count == 0 {
            return Err(ReadError::InvalidRange);
        }
        let byte_len = (sector_count as u64)
            .checked_mul(u64::from(sector_size))
            .ok_or(ReadError::InvalidRange)?;
        let byte_len = usize::try_from(byte_len).map_err(|_| ReadError::InvalidRange)?;
        if buffer.len() < byte_len {
            return Err(ReadError::InvalidRange);
        }
        let offset = sector_byte_offset(sector_index, sector_size)?;
        let mut receipt = self.read_at(offset, &mut buffer[..byte_len])?;
        receipt.address_space = AddressSpace::Storage;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::FileSource;
    use std::io::Write;
    use u_library_core::{compare_snapshots, AddressSpace};

    #[test]
    fn file_snapshot_round_trip_and_compare() {
        let dir = std::env::temp_dir().join("u-library-file-snapshot");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.bin");
        std::fs::File::create(&path)
            .unwrap()
            .write_all(&[10, 20, 30, 40])
            .unwrap();

        let mut source = FileSource::open(&path).unwrap();
        let first = source.snapshot_at(1, 3).unwrap();
        assert_eq!(first.address_space, AddressSpace::File);
        assert_eq!(first.address, 1);
        assert_eq!(first.bytes, [20, 30, 40]);

        std::fs::write(&path, [10, 21, 30, 41]).unwrap();
        let mut source = FileSource::open(&path).unwrap();
        let second = source.snapshot_at(1, 3).unwrap();
        let changes = compare_snapshots(&first, &second).unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].address, 1);
        assert_eq!(changes[0].before, 20);
        assert_eq!(changes[0].after, 21);
        assert_eq!(changes[1].address, 3);
        assert_eq!(changes[1].before, 40);
        assert_eq!(changes[1].after, 41);

        let source = FileSource::open(&path).unwrap();
        assert_eq!(source.size_bytes().unwrap(), 4);
        let mut sectors = [0u8; 512];
        std::fs::write(&path, vec![7u8; 512]).unwrap();
        let mut source = FileSource::open(&path).unwrap();
        let receipt = source
            .read_sectors(0, 512, 1, &mut sectors)
            .unwrap();
        assert_eq!(receipt.address_space, AddressSpace::Storage);
        assert_eq!(receipt.bytes_read, 512);
        assert_eq!(sectors[0], 7);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
