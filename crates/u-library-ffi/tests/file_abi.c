/* Link with the U-Library dynamic library and pass a three-byte file
 * containing abc as argv[1], and its parent directory as argv[2]. */
#include "u_library.h"
#include <assert.h>
#include <string.h>

int main(int argc, char **argv) {
    static uint8_t oversized[UL_MAX_READ_BYTES + 1];
    UL_FileHandle *file = NULL;
    UL_FileHandle *failed;
    uint8_t buffer[4] = {0, 0, 0, 42};
    size_t count = 99;

    assert(argc == 3);
    assert(ul_abi_version() == 1);
    assert(ul_file_open(argv[1], strlen(argv[1]), &file) == UL_OK);
    assert(file != NULL);
    failed = file;
    assert(ul_file_open(NULL, 1, &failed) == UL_INVALID_ARGUMENT);
    assert(failed == NULL);
    assert(ul_file_open("\xff", 1, &failed) == UL_INVALID_ARGUMENT);
    assert(ul_file_open("a\0b", 3, &failed) == UL_INVALID_ARGUMENT);
    assert(ul_file_open(argv[2], strlen(argv[2]), &failed) == UL_UNSUPPORTED_SOURCE);
    assert(ul_file_open(argv[1], strlen(argv[1]), NULL) == UL_INVALID_ARGUMENT);

    assert(ul_file_read_at(file, 0, buffer, 3, &count) == UL_OK);
    assert(count == 3 && memcmp(buffer, "abc", 3) == 0 && buffer[3] == 42);
    assert(ul_file_read_at(file, 1, buffer, sizeof(buffer), &count) == UL_OK);
    assert(count == 2 && memcmp(buffer, "bc", 2) == 0 && buffer[2] == 'c');
    assert(ul_file_read_at(file, 3, buffer, 1, &count) == UL_OK && count == 0);
    assert(ul_file_read_at(file, 10, buffer, 1, &count) == UL_OK && count == 0);
    assert(ul_file_read_at(file, UINT64_MAX, NULL, 0, &count) == UL_OK && count == 0);
    assert(ul_file_read_at(file, UINT64_MAX, buffer, 1, &count) == UL_INVALID_RANGE);
    assert(ul_file_read_at(file, 0, oversized, UL_MAX_READ_BYTES, &count) == UL_OK);
    assert(count == 3 && memcmp(oversized, "abc", 3) == 0);
    assert(ul_file_read_at(file, 0, oversized, sizeof(oversized), &count) == UL_INVALID_RANGE);
    assert(ul_file_read_at(file, 0, NULL, 1, &count) == UL_INVALID_ARGUMENT);
    assert(ul_file_read_at(file, 0, buffer, 1, NULL) == UL_INVALID_ARGUMENT);
    count = 99;
    assert(ul_file_read_at(NULL, 0, buffer, 1, &count) == UL_INVALID_ARGUMENT);
    assert(count == 0);
    ul_file_close(file);
    ul_file_close(NULL);
    return 0;
}
