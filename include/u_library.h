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
    UL_BUFFER_TOO_SMALL = 6,
    UL_PERMISSION_REQUIRED = 7,
    UL_UNAVAILABLE = 8,
    UL_MAX_READ_BYTES = 1048576
};

typedef struct UL_ByteDiff {
    uint64_t address;
    uint8_t before;
    uint8_t after;
    uint8_t reserved[6]; /* Always zero in version 1. */
} UL_ByteDiff;

uint32_t ul_abi_version(void);

/* Compares two equal-length buffers that describe the same base address.
 * Writes differing bytes to out when capacity is sufficient. Pass NULL and
 * zero capacity to obtain the required count. If capacity is too small,
 * returns UL_BUFFER_TOO_SMALL and sets count without writing to out.
 * Lengths must match and stay within UL_MAX_READ_BYTES. The exclusive end
 * of base_address + length must not overflow uint64_t. before/after may be
 * NULL only when the matching length is zero. */
UL_Status ul_bytes_compare(uint64_t base_address, const uint8_t *before,
                           size_t before_len, const uint8_t *after,
                           size_t after_len, UL_ByteDiff *out, size_t capacity,
                           size_t *count);

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

/* Writes the current file length in bytes. out_size is required and zero on
 * failure. */
UL_Status ul_file_size(const UL_FileHandle *handle, uint64_t *out_size);

/* Reads whole sectors. sector_size 0 means 512. The product
 * sector_count * sector_size must fit capacity and UL_MAX_READ_BYTES. */
UL_Status ul_file_read_sectors(const UL_FileHandle *handle,
                               uint64_t sector_index, uint32_t sector_size,
                               uint32_t sector_count, uint8_t *buffer,
                               size_t capacity, size_t *bytes_read);


/* Optional process extension. Its version is 1 on Windows, Linux, and macOS,
 * and 0 elsewhere. To load older binaries dynamically, look up this symbol
 * before calling any process function. ul_abi_version() still reports 1.
 * On macOS, opening a foreign process usually requires elevated rights. */
uint32_t ul_process_abi_version(void);

typedef struct UL_ProcessHandle UL_ProcessHandle;

enum {
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


/* Optional registers extension. Version is 1 when current-thread capture
 * (Windows x86-64) or other-thread capture (Windows x86-64 / Linux) is built,
 * otherwise 0. Other-thread capture requires explicit caller consent. */
uint32_t ul_registers_abi_version(void);

enum {
    UL_ARCH_UNKNOWN = 0,
    UL_ARCH_X86_64 = 1,
    UL_ARCH_AARCH64 = 2
};

typedef struct UL_Register {
    char name[16]; /* NUL-terminated ISA name, for example RAX or X0. */
    uint64_t bits;
    uint8_t width_bits;
    uint8_t reserved[7]; /* Always zero in version 1. */
} UL_Register;

/* Writes registers into out when capacity is sufficient. Pass NULL and zero
 * capacity to obtain the required count. architecture and thread_id are
 * required outputs and are zero on failure. */
UL_Status ul_registers_capture_current(UL_Register *out, size_t capacity,
                                       size_t *count, uint32_t *architecture,
                                       uint64_t *thread_id);

UL_Status ul_registers_capture_thread(uint64_t pid, uint64_t tid,
                                      UL_Register *out, size_t capacity,
                                      size_t *count, uint32_t *architecture,
                                      uint64_t *thread_id);


/* Optional cache extension. Version is 1 when topology discovery is built
 * (Windows / Linux), otherwise 0. Hardware counters are supported on Linux
 * when perf_event_open allows; Windows returns UL_UNAVAILABLE for counters. */
uint32_t ul_cache_abi_version(void);

enum {
    UL_CACHE_KIND_DATA = 1,
    UL_CACHE_KIND_INSTRUCTION = 2,
    UL_CACHE_KIND_UNIFIED = 3,
    UL_CACHE_KIND_TRACE = 4,
    UL_CACHE_KIND_UNKNOWN = 5
};

#define UL_CACHE_FLAG_FULLY_ASSOCIATIVE (1u << 0)
#define UL_CACHE_METRIC_UNAVAILABLE UINT64_MAX

typedef struct UL_CacheNode {
    uint32_t level;
    uint32_t kind; /* UL_CACHE_KIND_* */
    int64_t size_bytes; /* -1 when the OS does not report size */
    uint32_t line_size_bytes; /* 0 when unknown */
    uint32_t associativity; /* 0 when unknown */
    uint32_t flags; /* UL_CACHE_FLAG_* */
    uint32_t shared_cpu_count;
    uint32_t reserved; /* Always zero in version 1. */
} UL_CacheNode;

/* capacity and count are in records. Pass NULL and zero capacity to obtain
 * the required count. On other failures count is zero. */
UL_Status ul_cache_topology(UL_CacheNode *buffer, size_t capacity, size_t *count);

/* Samples a short PMC/perf window. On UL_OK, each output is a measured value
 * or UL_CACHE_METRIC_UNAVAILABLE when that counter was not reported. */
UL_Status ul_cache_counters(uint64_t *references, uint64_t *misses,
                            uint64_t *window_nanos);


/* Optional GPU extension. Version is 1 on Windows DXGI builds, otherwise 0.
 * Dedicated/shared bytes use UINT64_MAX when the adapter does not report them.
 * This API never claims access to arbitrary VRAM contents. */
uint32_t ul_gpu_abi_version(void);

typedef struct UL_GpuDevice {
    char name[128];
    uint64_t dedicated_bytes;
    uint64_t shared_bytes;
    uint32_t vendor_id;
    uint32_t device_id;
    uint8_t reserved[8];
} UL_GpuDevice;

UL_Status ul_gpu_list(UL_GpuDevice *buffer, size_t capacity, size_t *count);

#ifdef __cplusplus
}
#endif

#endif
