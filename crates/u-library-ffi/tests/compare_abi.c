/* Link with the U-Library dynamic library. */
#include "u_library.h"
#include <assert.h>
#include <string.h>

int main(void) {
    static const uint8_t before[] = {1, 2, 3, 4};
    static const uint8_t after[] = {1, 9, 3, 8};
    UL_ByteDiff diffs[4];
    UL_ByteDiff tiny[1];
    size_t count = 99;

    assert(ul_abi_version() == 1);
    assert(ul_bytes_compare(0x1000, before, 4, after, 4, NULL, 0, &count) ==
           UL_BUFFER_TOO_SMALL);
    assert(count == 2);
    assert(ul_bytes_compare(0x1000, before, 4, after, 4, diffs, 4, &count) == UL_OK);
    assert(count == 2);
    assert(diffs[0].address == 0x1001 && diffs[0].before == 2 && diffs[0].after == 9);
    assert(diffs[1].address == 0x1003 && diffs[1].before == 4 && diffs[1].after == 8);
    assert(diffs[0].reserved[0] == 0 && diffs[1].reserved[5] == 0);

    assert(ul_bytes_compare(0x1000, before, 4, after, 4, tiny, 1, &count) ==
           UL_BUFFER_TOO_SMALL);
    assert(count == 2);
    assert(ul_bytes_compare(0x1000, before, 4, before, 4, diffs, 4, &count) == UL_OK);
    assert(count == 0);
    assert(ul_bytes_compare(0x1000, before, 4, after, 3, diffs, 4, &count) ==
           UL_INVALID_ARGUMENT);
    assert(count == 0);
    assert(ul_bytes_compare(UINT64_MAX, before, 1, after, 1, diffs, 4, &count) ==
           UL_INVALID_RANGE);
    assert(ul_bytes_compare(0, before, 4, after, 4, diffs, 4, NULL) ==
           UL_INVALID_ARGUMENT);
    assert(ul_bytes_compare(0, NULL, 1, after, 1, diffs, 4, &count) ==
           UL_INVALID_ARGUMENT);
    assert(ul_bytes_compare(0, before, 0, after, 0, diffs, 4, &count) == UL_OK);
    assert(count == 0);
    return 0;
}
