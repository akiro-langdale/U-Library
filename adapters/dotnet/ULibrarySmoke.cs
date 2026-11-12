using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;

namespace ULibrary
{
    public static class Smoke
    {
        public static string Run(string directory)
        {
            byte[] payload = { 0x55, 0x00, 0xFF, 0x4C };
            string path = Path.Combine(directory, "u-library-\u0442\u0435\u0441\u0442-" + Guid.NewGuid() + ".bin");
            try
            {
                File.WriteAllBytes(path, payload);
                if (BitConverter.ToString(Client.ReadFile(path, 1, 8)) != "00-FF-4C" ||
                    Client.ReadFile(path, 20, 8).Length != 0)
                    throw new InvalidOperationException("Unexpected file read.");
            }
            finally { File.Delete(path); }

            ulong pid;
            using (Process current = Process.GetCurrentProcess())
                pid = (ulong)current.Id;

            GCHandle pinned = GCHandle.Alloc(payload, GCHandleType.Pinned);
            try
            {
                ulong address = unchecked((ulong)pinned.AddrOfPinnedObject().ToInt64());
                Client.Region[] regions = Client.GetRegions(pid);
                bool found = Array.Exists(regions, region => address >= region.BaseAddress &&
                    address - region.BaseAddress < region.SizeBytes && (region.Flags & 16) != 0);
                string bytes = BitConverter.ToString(Client.ReadProcess(pid, address, payload.Length));
                if (!found || bytes != "55-00-FF-4C")
                    throw new InvalidOperationException("Unexpected process map or bytes.");
                return "U-Library OK. Regions: " + regions.Length + ". Bytes: " + bytes.Replace('-', ' ');
            }
            finally { pinned.Free(); }
        }
    }
}
