using System;
using System.Runtime.InteropServices;
using System.Text;

namespace ULibrary
{
    public sealed class NativeException : Exception
    {
        public int Status { get; private set; }

        internal NativeException(string operation, int status)
            : base(operation + " returned UL_Status " + status)
        {
            Status = status;
        }
    }

    public static class Client
    {
        public const int MaxFileRead = 1048576;
        public const int MaxProcessRead = 512;
        private const int BufferTooSmall = 6;
        private const string Library = "u_library";

        [StructLayout(LayoutKind.Sequential)]
        public struct Region
        {
            public ulong BaseAddress;
            public ulong SizeBytes;
            public uint State;
            public uint Kind;
            public uint Flags;
            public uint Reserved;
        }

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern uint ul_abi_version();

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern int ul_file_open(byte[] path, UIntPtr length, out IntPtr handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern int ul_file_read_at(IntPtr handle, ulong offset,
            [Out] byte[] buffer, UIntPtr capacity, out UIntPtr count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern void ul_file_close(IntPtr handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern uint ul_process_abi_version();

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern int ul_process_open(ulong pid, out IntPtr handle);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern int ul_process_regions(IntPtr handle,
            [Out] Region[] buffer, UIntPtr capacity, out UIntPtr count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern int ul_process_read_at(IntPtr handle, ulong address,
            [Out] byte[] buffer, UIntPtr capacity, out UIntPtr count);

        [DllImport(Library, CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
        private static extern void ul_process_close(IntPtr handle);

        public static uint AbiVersion { get { return ul_abi_version(); } }

        public static uint ProcessAbiVersion
        {
            get
            {
                try { return ul_process_abi_version(); }
                catch (EntryPointNotFoundException) { return 0; }
            }
        }

        public static byte[] ReadFile(string path, ulong offset, int length)
        {
            if (string.IsNullOrEmpty(path) || path.IndexOf('\0') >= 0)
                throw new ArgumentException("Expected a nonempty filesystem path.", "path");
            if (length < 0 || length > MaxFileRead)
                throw new ArgumentOutOfRangeException("length");
            RequireAbi();
            byte[] utf8 = new UTF8Encoding(false, true).GetBytes(path);
            byte[] bytes = new byte[length];
            IntPtr handle;
            Check(ul_file_open(utf8, Size(utf8.Length), out handle), "ul_file_open");
            try
            {
                UIntPtr count;
                Check(ul_file_read_at(handle, offset, bytes, Size(length), out count), "ul_file_read_at");
                Array.Resize(ref bytes, checked((int)count.ToUInt64()));
                return bytes;
            }
            finally { ul_file_close(handle); }
        }

        public static Region[] GetRegions(ulong pid)
        {
            IntPtr handle = OpenProcess(pid);
            try
            {
                Region[] regions = new Region[0];
                // The map can grow between calls, including due to allocating the buffer.
                for (int attempt = 0; attempt < 8; ++attempt)
                {
                    UIntPtr count;
                    int status = ul_process_regions(handle, regions.Length == 0 ? null : regions,
                        Size(regions.Length), out count);
                    if (status == BufferTooSmall)
                    {
                        regions = new Region[checked((int)count.ToUInt64() + 64)];
                        continue;
                    }
                    Check(status, "ul_process_regions");
                    Array.Resize(ref regions, checked((int)count.ToUInt64()));
                    return regions;
                }
                throw new NativeException("ul_process_regions", BufferTooSmall);
            }
            finally { ul_process_close(handle); }
        }

        public static byte[] ReadProcess(ulong pid, ulong address, int length)
        {
            if (length < 1 || length > MaxProcessRead)
                throw new ArgumentOutOfRangeException("length");
            byte[] bytes = new byte[length];
            IntPtr handle = OpenProcess(pid);
            try
            {
                UIntPtr count;
                Check(ul_process_read_at(handle, address, bytes, Size(length), out count), "ul_process_read_at");
                if (count.ToUInt64() != (ulong)length)
                    throw new InvalidOperationException("Unexpected process read size.");
                return bytes;
            }
            finally { ul_process_close(handle); }
        }

        private static IntPtr OpenProcess(ulong pid)
        {
            RequireAbi();
            if (ProcessAbiVersion != 1)
                throw new NotSupportedException("U-Library process ABI v1 is required.");
            IntPtr handle;
            Check(ul_process_open(pid, out handle), "ul_process_open");
            return handle;
        }

        private static void RequireAbi()
        {
            if (AbiVersion != 1)
                throw new NotSupportedException("U-Library file ABI v1 is required.");
        }

        private static UIntPtr Size(int value) { return new UIntPtr((uint)value); }

        private static void Check(int status, string operation)
        {
            if (status != 0)
                throw new NativeException(operation, status);
        }
    }
}
