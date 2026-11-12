"""Inspect the current Python process through the U-Library C ABI."""

import argparse
import ctypes as c
import os
from pathlib import Path


BUFFER_TOO_SMALL = 6


class Region(c.Structure):
    _fields_ = [
        ("base_address", c.c_uint64),
        ("size_bytes", c.c_uint64),
        ("state", c.c_uint32),
        ("kind", c.c_uint32),
        ("flags", c.c_uint32),
        ("reserved", c.c_uint32),
    ]


def check(status, operation):
    if status:
        raise RuntimeError(f"{operation} returned {status}")


def inspect(dll_path):
    library = c.CDLL(str(dll_path.resolve()))
    library.ul_process_abi_version.argtypes = []
    library.ul_process_abi_version.restype = c.c_uint32
    if library.ul_process_abi_version() != 1:
        raise RuntimeError("Process ABI v1 is required.")

    library.ul_process_open.argtypes = [c.c_uint64, c.POINTER(c.c_void_p)]
    library.ul_process_open.restype = c.c_int32
    library.ul_process_regions.argtypes = [
        c.c_void_p, c.POINTER(Region), c.c_size_t, c.POINTER(c.c_size_t)
    ]
    library.ul_process_regions.restype = c.c_int32
    library.ul_process_read_at.argtypes = [
        c.c_void_p, c.c_uint64, c.POINTER(c.c_uint8), c.c_size_t, c.POINTER(c.c_size_t)
    ]
    library.ul_process_read_at.restype = c.c_int32
    library.ul_process_close.argtypes = [c.c_void_p]
    library.ul_process_close.restype = None

    process = c.c_void_p()
    check(library.ul_process_open(os.getpid(), c.byref(process)), "ul_process_open")
    try:
        count = c.c_size_t()
        status = library.ul_process_regions(process, None, 0, c.byref(count))
        while status == BUFFER_TOO_SMALL:
            regions = (Region * count.value)()
            status = library.ul_process_regions(process, regions, len(regions), c.byref(count))
        check(status, "ul_process_regions")
        print(f"Regions: {count.value}")

        payload = (c.c_uint8 * 4)(0x55, 0, 0xFF, 0x4C)
        sample = (c.c_uint8 * len(payload))()
        bytes_read = c.c_size_t()
        check(
            library.ul_process_read_at(
                process, c.addressof(payload), sample, len(sample), c.byref(bytes_read)
            ),
            "ul_process_read_at",
        )
        if bytes_read.value != len(payload) or bytes(sample) != bytes(payload):
            raise RuntimeError("Unexpected sample.")
        print("Bytes:", " ".join(f"{value:02X}" for value in sample))
    finally:
        library.ul_process_close(process)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dll", type=Path, help="Path to u_library.dll")
    args = parser.parse_args()
    try:
        inspect(args.dll)
    except (OSError, AttributeError, RuntimeError) as error:
        parser.exit(1, f"{error}\n")
