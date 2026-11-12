#include "u_library.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>

static void check_region_boundary(const UL_ProcessHandle *process) {
    SYSTEM_INFO system;
    GetSystemInfo(&system);
    const size_t page_size = system.dwPageSize;
    uint8_t *pages = VirtualAlloc(NULL, 2 * page_size, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE);
    assert(pages != NULL);
    DWORD old_protection;
    assert(VirtualProtect(pages + page_size, page_size, PAGE_NOACCESS, &old_protection));
    uint8_t *end = pages + page_size - UL_MAX_PROCESS_READ_BYTES;
    memset(end, 0x55, UL_MAX_PROCESS_READ_BYTES);
    const uint64_t address = (uint64_t)(uintptr_t)end;
    uint8_t sample[UL_MAX_PROCESS_READ_BYTES + 1] = {0};
    size_t bytes_read = 0;

    assert(ul_process_read_at(process, address, sample, UL_MAX_PROCESS_READ_BYTES, &bytes_read) == UL_OK);
    assert(bytes_read == UL_MAX_PROCESS_READ_BYTES);
    assert(memcmp(sample, end, bytes_read) == 0);
    assert(sample[UL_MAX_PROCESS_READ_BYTES] == 0);

    sample[0] = 0xAA;
    sample[1] = 0xBB;
    assert(ul_process_read_at(process, address + UL_MAX_PROCESS_READ_BYTES - 1, sample, 2, &bytes_read) == UL_UNAVAILABLE);
    assert(bytes_read == 0 && sample[0] == 0xAA && sample[1] == 0xBB);
    assert(ul_process_read_at(process, address, sample, sizeof(sample), &bytes_read) == UL_INVALID_RANGE);
    assert(bytes_read == 0 && sample[0] == 0xAA && sample[1] == 0xBB);
    assert(VirtualFree(pages, 0, MEM_RELEASE));
}
#endif

int main(void) {
    UL_ProcessHandle *process = NULL;

    assert(ul_abi_version() == 1);
    assert(ul_process_open(0, NULL) == UL_INVALID_ARGUMENT);

#ifdef _WIN32
    static const uint8_t payload[] = {0x75, 0x00, 0xFF, 0x4C};
    const uint64_t address = (uint64_t)(uintptr_t)payload;
    uint8_t sample[sizeof(payload)] = {0};
    size_t bytes_read = 0;
    size_t count = 0;
    UL_ProcessRegion *regions = NULL;
    UL_Status status;
    int found = 0;

    assert(sizeof(UL_ProcessRegion) == 32);
    assert(offsetof(UL_ProcessRegion, flags) == 24);
    assert(ul_process_abi_version() == 1);
    assert(ul_process_open(UINT64_MAX, &process) == UL_UNAVAILABLE);
    assert(process == NULL);
    assert(ul_process_open(GetCurrentProcessId(), &process) == UL_OK);
    assert(process != NULL);

    status = ul_process_regions(process, NULL, 0, &count);
    while (status == UL_BUFFER_TOO_SMALL) {
        assert(count > 0 && count <= SIZE_MAX / sizeof(*regions));
        free(regions);
        regions = malloc(count * sizeof(*regions));
        assert(regions != NULL);
        status = ul_process_regions(process, regions, count, &count);
    }
    assert(status == UL_OK);
    for (size_t i = 0; i < count; ++i) {
        const UL_ProcessRegion *region = &regions[i];
        assert(region->reserved == 0);
        if (address >= region->base_address &&
            address - region->base_address < region->size_bytes) {
            assert(region->state == UL_REGION_STATE_COMMITTED);
            assert((region->flags & UL_REGION_SAMPLE_READABLE) != 0);
            found = 1;
        }
    }
    assert(found);
    free(regions);

    assert(ul_process_read_at(process, address, sample, sizeof(sample), &bytes_read) == UL_OK);
    assert(bytes_read == sizeof(payload));
    assert(memcmp(sample, payload, sizeof(payload)) == 0);

    assert(ul_process_read_at(process, UINT64_MAX, sample, sizeof(sample), &bytes_read) == UL_INVALID_RANGE);
    assert(bytes_read == 0 && memcmp(sample, payload, sizeof(payload)) == 0);
    assert(ul_process_read_at(process, 0, sample, sizeof(sample), &bytes_read) == UL_UNAVAILABLE);
    assert(bytes_read == 0 && memcmp(sample, payload, sizeof(payload)) == 0);
    check_region_boundary(process);
    ul_process_close(process);
#else
    assert(ul_process_abi_version() == 0);
    assert(ul_process_open(1, &process) == UL_UNSUPPORTED_SOURCE);
    assert(process == NULL);
#endif
    ul_process_close(NULL);
    return 0;
}

