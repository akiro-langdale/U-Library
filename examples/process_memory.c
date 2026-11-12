#include "u_library.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

int main(void) {
    static const uint8_t payload[] = {0x55, 0x00, 0xFF, 0x4C};
    uint8_t sample[sizeof(payload)];
    UL_ProcessHandle *process = NULL;
    UL_ProcessRegion *regions = NULL;
    size_t count = 0;
    size_t bytes_read = 0;
    int result = EXIT_FAILURE;
    UL_Status status;

    if (ul_process_abi_version() != 1) {
        fputs("Process ABI v1 is required.\n", stderr);
        return result;
    }

    status = ul_process_open(GetCurrentProcessId(), &process);
    if (status != UL_OK) {
        fprintf(stderr, "ul_process_open: %d\n", status);
        return result;
    }

    status = ul_process_regions(process, NULL, 0, &count);
    while (status == UL_BUFFER_TOO_SMALL) {
        if (count > SIZE_MAX / sizeof(*regions)) {
            fputs("Region array is too large.\n", stderr);
            goto cleanup;
        }
        UL_ProcessRegion *next = (UL_ProcessRegion *)realloc(regions, count * sizeof(*regions));
        if (next == NULL) {
            fputs("Could not allocate region array.\n", stderr);
            goto cleanup;
        }
        regions = next;
        status = ul_process_regions(process, regions, count, &count);
    }
    if (status != UL_OK) {
        fprintf(stderr, "ul_process_regions: %d\n", status);
        goto cleanup;
    }
    printf("Regions: %zu\n", count);

    status = ul_process_read_at(process, (uint64_t)(uintptr_t)payload,
                                sample, sizeof(sample), &bytes_read);
    if (status != UL_OK) {
        fprintf(stderr, "ul_process_read_at: %d\n", status);
        goto cleanup;
    }
    if (bytes_read != sizeof(payload) || memcmp(sample, payload, sizeof(payload)) != 0) {
        fputs("Unexpected sample.\n", stderr);
        goto cleanup;
    }
    printf("Bytes:");
    for (size_t i = 0; i < bytes_read; ++i) {
        printf(" %02X", (unsigned int)sample[i]);
    }
    puts("");
    result = EXIT_SUCCESS;

cleanup:
    free(regions);
    ul_process_close(process);
    return result;
}
