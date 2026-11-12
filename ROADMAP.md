# U-Library / U-Monitor roadmap

## Done

- Library-first architecture: U-Monitor consumes U-Library
- File reads, snapshots, byte search/compare
- Process virtual memory (Windows / Linux / macOS)
- CPU registers (Windows x86-64 current + other thread; Linux ptrace)
- Cache topology (Windows / Linux) and Linux perf counters
- Storage sector reads on top of `FileSource`
- GPU adapter inventory (Windows DXGI; no VRAM contents)
- C ABI extensions + engine adapters (Unity / UE / Godot)

## Intentionally Monitor-only

- Live telemetry worker (CPU/RAM/disk rates UI polling)
- Educational hierarchy simulation (`SIMULATED`)
- egui frontend, themes, overview dashboard

## Remaining polish

- Push `develop` → `master` after green CI on a pinned U-Library revision
- Expand FFI feature parity (`um_*` vs `PlatformFeature`)
- Optional Linux/macOS GPU backends when a stable query API is chosen
- Hardware validation matrix on real Win/Linux/macOS ARM64 hosts
