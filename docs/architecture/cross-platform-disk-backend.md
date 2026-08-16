# Architecture: cross-platform raw disk access

## What changed

Before this session, `recovar-core`'s disk module only worked on Windows. Every other
platform compiled to `disk::stub`, which returns an empty drive list and a hard error on
any attempt to open a drive. The README only ever claimed Windows support, so this wasn't a
regression, but the app could not do laptop recovery anywhere except Windows.

Added two new platform backends implementing the same `DiskReader` trait and
`list_drives`/`open_drive` functions the Windows backend already provides:

- `recovar-core/src/disk/linux.rs` — enumerates block devices via `/sys/class/block`
  (skipping loop, ram, device-mapper, and software-RAID entries, which are not meaningful
  recovery targets), reads exact size and sector size via the `BLKGETSIZE64`/`BLKSSZGET`
  ioctls, and opens `/dev/<name>` with `std::os::unix::fs::FileExt::read_at` for positioned
  reads.
- `recovar-core/src/disk/macos.rs` — enumerates `/dev/diskN` and `/dev/diskNsM` entries via
  `readdir`, reads exact size via the IOKit `DKIOCGETBLOCKSIZE`/`DKIOCGETBLOCKCOUNT` ioctls,
  and opens the device the same way.

`recovar-cli`'s Administrator check (`is_admin`, Windows-only via `IsUserAnAdmin`) was
generalized to `has_disk_privileges()` / `elevation_hint()`, with a Unix implementation via
`libc::geteuid() == 0`. The "run as Administrator" warning is now also scoped to laptop scans
only — it printed unconditionally before, including for Android scans, which don't need
elevated privileges.

## Why this approach

Windows needed `CreateFileW` plus manual sector alignment because it opens the volume with
`FILE_FLAG_NO_BUFFERING` for direct unbuffered access. Linux and macOS don't need that
complexity: opening the raw device node as a normal buffered file and reading at arbitrary
offsets via `pread` (exposed as `FileExt::read_at`) works correctly without any alignment
requirements, because these platforms don't require O_DIRECT-style alignment for correctness,
only (optionally) for performance. This kept both new backends short and free of the
alignment bookkeeping the Windows backend needs.

Filesystem type detection (NTFS vs FAT32) is deliberately left as `"Unknown"` in
`list_drives()` on both new platforms, matching the existing fallback the Windows backend
already uses when `GetVolumeInformationW` fails. The real filesystem detection that matters
happens in `RecoverySession::run_laptop`, which reads the volume's own boot sector directly —
this already works identically on every platform once `open_drive`/`read_at` work.

## Files affected

- `recovar-core/src/disk/mod.rs` (dispatch by `target_os`)
- `recovar-core/src/disk/linux.rs` (new)
- `recovar-core/src/disk/macos.rs` (new)
- `recovar-core/src/disk/stub.rs` (now only compiled for platforms that are neither Windows,
  Linux, nor macOS — e.g. BSD)
- `recovar-core/Cargo.toml` (added `libc` as a `cfg(unix)` dependency)
- `recovar-cli/src/main.rs`, `recovar-cli/Cargo.toml` (cross-platform elevation check)

## Verification

No Linux or macOS machine was available in this session — only Windows. What was actually
verified:

- `cargo build --release` for the full workspace on Windows still succeeds unchanged (the new
  modules are `cfg`-gated out).
- Installed the `x86_64-unknown-linux-gnu` and `x86_64-apple-darwin` Rust targets via
  `rustup target add` and ran `cargo check -p recovar-core -p recovar-cli --target <target>`
  for both. This performs full type checking (not just parsing) against each platform's real
  standard library and the real `libc` crate, including every `unsafe` ioctl call, without
  needing a linker for that target. Both platforms type-check cleanly, including
  `recovar-core`'s test suite under the Linux target.

What was **not** verified: actually running the compiled binary against a real Linux or macOS
block device, or building a fully linked binary for either platform (that needs a linker
toolchain and, for macOS, typically the actual OS or a proper cross-linker setup, neither of
which is available here). The ioctl request-code constants (`BLKGETSIZE64`, `BLKSSZGET`,
`DKIOCGETBLOCKCOUNT`, `DKIOCGETBLOCKSIZE`) are the standard, widely-used values for these
requests and were entered from their well-known definitions, but their exact runtime
behavior against a real device has not been observed. Treat the Linux and macOS backends as
implemented and type-correct, not yet field-tested.
