/// Windows virtual memory map and bounded read-only samples.
#[cfg(windows)]
pub mod process_memory;

use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    time::SystemTime,
};
use u_library_core::{AddressSpace, ReadError, ReadReceipt, MAX_READ_BYTES};

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
}
