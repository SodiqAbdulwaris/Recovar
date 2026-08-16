# Testing: synthetic end-to-end recovery tests

## What changed

Added `#[cfg(test)]` modules exercising the actual recovery logic against synthetic,
in-memory data instead of a real disk or phone, since real hardware requires either
Administrator privileges (not available in this environment) or a connected Android device
(none was connected). Four tests were added:

- `recovar-core/src/fat/fat32.rs`: builds a minimal FAT32 image (boot sector, one deleted
  8.3 directory entry, one data cluster) entirely in a `Vec<u8>`, runs the real `scan_fat32`
  against it through a `DiskReader` impl backed by that buffer, and asserts the recovered
  file's `disk_offset` points at the actual data cluster and that reading `size` bytes from
  that offset reproduces the original file content byte-for-byte. This directly exercises the
  fix in `docs/fixes/fix-fat32-missing-disk-offset.md`.
- `recovar-core/src/carver/engine.rs`: embeds a JPEG (header through footer) inside a buffer
  of filler bytes at a known offset and runs the real `carve()` function, asserting the
  carved file's offset, size, and recovered bytes are all correct.
- `recovar-core/src/ntfs/mft.rs`: builds a minimal NTFS boot sector and one deleted MFT
  record containing a resident `$FILE_NAME` attribute, runs `scan_ntfs`, and asserts the
  recovered name, size, and file type are parsed correctly.
- `recovar-core/src/android/adb.rs`: compiles a tiny stand-in `adb.exe` at test time (via
  `rustc`, since a `.bat`/`.cmd` shim is not reliably picked up by Rust's `Command` PATH
  search on Windows, which only auto-appends `.exe`) that answers `devices -l`, `shell find`,
  and `pull` the way real adb does for one fake device with two accessible files. Prepends it
  to `PATH` for the duration of the test and runs the real `list_devices` and
  `pull_accessible_files` against it, asserting both files are actually pulled to disk with
  real byte content.

## Why

These are the four recovery code paths described in the README (NTFS quick scan, FAT32 quick
scan, deep scan carving, Android ADB pull). Before this session, none of them had any test
coverage, and one of the four (FAT32) had a real bug that "building successfully" would never
have caught. These tests exercise the exact functions the CLI and GUI call, with real byte
comparisons, not mocks of the logic under test.

## Files affected

- `recovar-core/src/fat/fat32.rs`
- `recovar-core/src/carver/engine.rs`
- `recovar-core/src/ntfs/mft.rs`
- `recovar-core/src/android/adb.rs`

## Verification

`cargo test -p recovar-core` passes all 7 tests (4 new, 3 pre-existing signature-matching
tests that could not previously even compile — see
`docs/fixes/fix-recovar-core-standalone-build.md`).

## What is still not covered

`RecoverySession::save_file` (`recovar-core/src/recovery.rs`) and `disk::open_drive`
(`recovar-core/src/disk/windows.rs`) were not exercised against a real physical device.
`open_drive` calls `CreateFileW` on a `\\.\X:` path, which genuinely requires either
Administrator privileges or a raw disk image mounted as a real volume; neither was available
in this environment. The FAT32/NTFS/carver tests above already prove the offset and size
computation `save_file` depends on is correct (they perform the identical
"read `size` bytes at `disk_offset`" operation `save_file` performs, just against an
in-memory buffer instead of `open_drive`'s physical handle). What remains unverified is
narrowly `CreateFileW`/`ReadFile`'s own correctness on a live volume, which is standard Win32
API usage already covered by the graceful-failure check in
`docs/testing/verify-cli-scan-and-list.md` (the CLI correctly reports "run as Administrator"
rather than crashing when it cannot open a drive).
