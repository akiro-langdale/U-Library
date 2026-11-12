using System;
using System.Diagnostics;
using System.Runtime.InteropServices;

internal static class ProcessMemory
{
    private const string Library = "u_library.dll";
    private const int BufferTooSmall = 6;

    [StructLayout(LayoutKind.Sequential)]
    private struct Region
    {
        public ulong BaseAddress;
        public ulong SizeBytes;
        public uint State;
        public uint Kind;
        public uint Flags;
        public uint Reserved;
    }

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern uint ul_process_abi_version();

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern int ul_process_open(ulong pid, out IntPtr handle);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern int ul_process_regions(
        IntPtr handle, [Out] Region[] regions, UIntPtr capacity, out UIntPtr count);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern int ul_process_read_at(
        IntPtr handle, ulong address, [Out] byte[] buffer, UIntPtr capacity, out UIntPtr bytesRead);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    private static extern void ul_process_close(IntPtr handle);

    private static void Check(int status, string operation)
    {
        if (status != 0)
            throw new InvalidOperationException(operation + " returned " + status);
    }

    private static int Main()
    {
        try
        {
            if (ul_process_abi_version() != 1)
                throw new InvalidOperationException("Process ABI v1 is required.");

            IntPtr handle;
            using (Process current = Process.GetCurrentProcess())
                Check(ul_process_open((ulong)current.Id, out handle), "ul_process_open");

            try
            {
                UIntPtr count;
                int status = ul_process_regions(handle, null, UIntPtr.Zero, out count);
                while (status == BufferTooSmall)
                {
                    var regions = new Region[checked((int)count.ToUInt64())];
                    status = ul_process_regions(handle, regions, count, out count);
                }
                Check(status, "ul_process_regions");
                Console.WriteLine("Regions: " + count.ToUInt64());

                byte[] payload = { 0x55, 0x00, 0xFF, 0x4C };
                byte[] sample = new byte[payload.Length];
                GCHandle pinned = GCHandle.Alloc(payload, GCHandleType.Pinned);
                try
                {
                    IntPtr pointer = pinned.AddrOfPinnedObject();
                    ulong address = IntPtr.Size == 4
                        ? unchecked((uint)pointer.ToInt32())
                        : unchecked((ulong)pointer.ToInt64());
                    UIntPtr bytesRead;
                    Check(ul_process_read_at(handle, address, sample,
                        new UIntPtr((uint)sample.Length), out bytesRead), "ul_process_read_at");
                    if (bytesRead.ToUInt64() != (ulong)payload.Length ||
                        BitConverter.ToString(sample) != BitConverter.ToString(payload))
                        throw new InvalidOperationException("Unexpected sample.");

                    Console.WriteLine("Bytes: " + BitConverter.ToString(sample).Replace('-', ' '));
                }
                finally
                {
                    pinned.Free();
                }
            }
            finally
            {
                ul_process_close(handle);
            }
            return 0;
        }
        catch (Exception error)
        {
            Console.Error.WriteLine(error.Message);
            return 1;
        }
    }
}
