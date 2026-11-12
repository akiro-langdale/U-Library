#pragma once

#include "u_library.h"

#include <memory>
#include <string>
#include <vector>

namespace ULibrary
{
    inline UL_Status ReadFile(const std::string& path, uint64_t offset, size_t length,
                              std::vector<uint8_t>& bytes)
    {
        bytes.clear();
        if (length > UL_MAX_READ_BYTES)
            return UL_INVALID_RANGE;
        if (ul_abi_version() != 1)
            return UL_UNSUPPORTED_SOURCE;

        UL_FileHandle* raw = nullptr;
        UL_Status status = ul_file_open(path.data(), path.size(), &raw);
        if (status != UL_OK)
            return status;
        std::unique_ptr<UL_FileHandle, decltype(&ul_file_close)> handle(raw, ul_file_close);
        bytes.resize(length);
        size_t count = 0;
        status = ul_file_read_at(handle.get(), offset, bytes.data(), length, &count);
        bytes.resize(status == UL_OK ? count : 0);
        return status;
    }

    inline UL_Status GetRegions(uint64_t pid, std::vector<UL_ProcessRegion>& regions)
    {
        regions.clear();
        if (ul_abi_version() != 1 || ul_process_abi_version() != 1)
            return UL_UNSUPPORTED_SOURCE;

        UL_ProcessHandle* raw = nullptr;
        UL_Status status = ul_process_open(pid, &raw);
        if (status != UL_OK)
            return status;
        std::unique_ptr<UL_ProcessHandle, decltype(&ul_process_close)> handle(raw, ul_process_close);
        // Allow for a changing map and for allocations made by the query itself.
        for (int attempt = 0; attempt < 8; ++attempt)
        {
            size_t count = 0;
            status = ul_process_regions(handle.get(), regions.empty() ? nullptr : regions.data(),
                                        regions.size(), &count);
            if (status == UL_BUFFER_TOO_SMALL)
            {
                if (regions.max_size() < 64 || count > regions.max_size() - 64)
                    break;
                regions.resize(count + 64);
                continue;
            }
            regions.resize(status == UL_OK ? count : 0);
            return status;
        }
        regions.clear();
        return UL_BUFFER_TOO_SMALL;
    }

    inline UL_Status ReadProcess(uint64_t pid, uint64_t address, size_t length,
                                 std::vector<uint8_t>& bytes)
    {
        bytes.clear();
        if (length == 0 || length > UL_MAX_PROCESS_READ_BYTES)
            return UL_INVALID_RANGE;
        if (ul_abi_version() != 1 || ul_process_abi_version() != 1)
            return UL_UNSUPPORTED_SOURCE;

        UL_ProcessHandle* raw = nullptr;
        UL_Status status = ul_process_open(pid, &raw);
        if (status != UL_OK)
            return status;
        std::unique_ptr<UL_ProcessHandle, decltype(&ul_process_close)> handle(raw, ul_process_close);
        bytes.resize(length);
        size_t count = 0;
        status = ul_process_read_at(handle.get(), address, bytes.data(), length, &count);
        if (status == UL_OK && count != length)
            status = UL_INTERNAL_ERROR;
        if (status != UL_OK)
            bytes.clear();
        return status;
    }
}
