#ifndef U_LIBRARY_H
#define U_LIBRARY_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct UL_FileHandle UL_FileHandle;
typedef int32_t UL_Status;

enum {
    UL_OK = 0,
    UL_INVALID_ARGUMENT = 1,
    UL_INVALID_RANGE = 2,
    UL_UNSUPPORTED_SOURCE = 3,
    UL_IO_ERROR = 4,
    UL_INTERNAL_ERROR = 5,
    UL_MAX_READ_BYTES = 1048576
};

uint32_t ul_abi_version(void);

/* Opens a regular file read-only. path contains path_len bytes of UTF-8
 * without embedded NUL. A terminating NUL is unnecessary and excluded
 * from path_len. The library does not retain the path.
 * out is required and receives NULL on failure or an owned handle on success.
 * Pointer arguments must refer to valid, correctly aligned, nonoverlapping
 * storage for the duration of each call. */
UL_Status ul_file_open(const char *path, size_t path_len, UL_FileHandle **out);

/* Reads at a byte offset. At most UL_MAX_READ_BYTES may be requested.
 * A range whose exclusive end overflows uint64_t is invalid.
 * EOF or an offset past EOF returns UL_OK with a short read or zero bytes.
 * buffer is caller-owned and may be NULL when capacity is zero.
 * bytes_read is required and is zero on failure. Only the first *bytes_read
 * buffer bytes are written. Reads on the same live handle are serialized. */
UL_Status ul_file_read_at(const UL_FileHandle *handle, uint64_t offset,
                         uint8_t *buffer, size_t capacity, size_t *bytes_read);

/* Close each successful handle once, after all calls using it finish.
 * NULL is accepted. All copies of a closed handle become invalid. */
void ul_file_close(UL_FileHandle *handle);


/* Optional process extension. Its version is 1 on Windows and 0 where
 * unavailable. To load older binaries dynamically, look up this symbol
 * before calling any process function. ul_abi_version() still reports 1. */
uint32_t ul_process_abi_version(void);

typedef struct UL_ProcessHandle UL_ProcessHandle;

enum {
    UL_BUFFER_TOO_SMALL = 6,
    UL_PERMISSION_REQUIRED = 7,
    UL_UNAVAILABLE = 8,
    UL_MAX_PROCESS_READ_BYTES = 512
};

enum {
    UL_REGION_STATE_UNKNOWN = 0,
    UL_REGION_STATE_FREE = 1,
    UL_REGION_STATE_RESERVED = 2,
    UL_REGION_STATE_COMMITTED = 3
};

enum {
    UL_REGION_KIND_UNKNOWN = 0,
    UL_REGION_KIND_FREE = 1,
    UL_REGION_KIND_PRIVATE = 2,
    UL_REGION_KIND_IMAGE = 3,
    UL_REGION_KIND_MAPPED = 4
};

enum {
    UL_REGION_READ = 1u << 0,
    UL_REGION_WRITE = 1u << 1,
    UL_REGION_EXECUTE = 1u << 2,
    UL_REGION_GUARD = 1u << 3,
    UL_REGION_SAMPLE_READABLE = 1u << 4
};

typedef struct UL_ProcessRegion {
    uint64_t base_address;
    uint64_t size_bytes;
    uint32_t state;
    uint32_t kind;
    uint32_t flags;
    uint32_t reserved; /* Always zero in version 1. */
} UL_ProcessRegion;

/* Opens a process for querying and reading. out receives NULL on failure.
 * On unsupported platforms, a valid call returns UL_UNSUPPORTED_SOURCE.
 * All process pointer arguments follow the storage rules for ul_file_open.
 * Callers must synchronize close with every use of the handle. */
UL_Status ul_process_open(uint64_t pid, UL_ProcessHandle **out);

/* capacity and count are in records. Pass NULL and zero capacity to obtain
 * the required count. If capacity is too small, returns UL_BUFFER_TOO_SMALL
 * and sets count to the required size without writing to buffer. Each call
 * queries the current map, so retry if it grows. On success count is the
 * number of records written. On other failures count is zero. */
UL_Status ul_process_regions(const UL_ProcessHandle *handle,
                             UL_ProcessRegion *buffer, size_t capacity,
                             size_t *count);

/* Reads exactly capacity bytes within a single readable region. capacity
 * must be in 1..UL_MAX_PROCESS_READ_BYTES. On failure, bytes_read is zero
 * and buffer is untouched. Both output pointers are required. */
UL_Status ul_process_read_at(const UL_ProcessHandle *handle, uint64_t address,
                             uint8_t *buffer, size_t capacity, size_t *bytes_read);

/* Close each successful handle once, after all calls using it finish.
 * NULL is accepted. All copies of a closed handle become invalid. */
void ul_process_close(UL_ProcessHandle *handle);

#ifdef __cplusplus
}
#endif

#endif
