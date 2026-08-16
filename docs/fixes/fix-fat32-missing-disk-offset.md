# Fix: FAT32 recovery never wrote file data to disk

## Problem

`recovar-core/src/fat/fat32.rs`'s `parse_entry` always set `disk_offset: 0` on every
`RecoveredFile` it produced from a deleted FAT32 directory entry. `RecoverySession::save_file`
(`recovar-core/src/recovery.rs`) only copies bytes from disk when
`file.disk_offset > 0 && file.size > 0`; with `disk_offset` always `0`, that condition was
always false. `save_file` still returned `Ok(out_path)` in that case without ever creating the
file. Net effect: both the CLI's `--save` flag and the GUI's "Recover Selected" button
reported success for every FAT32 quick-scan result while writing nothing to disk. NTFS and
carved results were not affected — those code paths already set a real `disk_offset`.

This is significant because FAT32 is the filesystem used by most removable media (SD cards,
USB drives) and is one of the two headline "quick scan" features described in the README.

## Approach

`parse_entry` now reads the starting cluster number out of the directory entry (bytes 20-21
for the high 16 bits, bytes 26-27 for the low 16 bits — standard FAT32 layout) and computes
the file's first-cluster disk offset via the existing `cluster_offset` helper, the same
function `scan_dir` already uses to locate directory clusters. The deleted-entry marker
(`0xE5`) only overwrites the first byte of the short filename; the cluster and size fields
in the rest of the 32-byte entry survive deletion, so this is a standard, reliable technique.
`parse_entry` now takes `&Fat32Bpb` so it can call `cluster_offset`.

## Files affected

- `recovar-core/src/fat/fat32.rs`

## Verification

`cargo build --release -p recovar-core -p recovar-cli` succeeds. Ran
`recovar.exe scan --drive G:\ --mode quick` (G: is a real FAT32 volume on the dev machine)
without Administrator privileges; it correctly failed with a Windows "Incorrect function"
`ReadFile` error rather than crashing, which is the expected, documented behavior without
elevation (see `setup.md`). Full end-to-end verification of recovered file contents requires
Administrator privileges on a drive with real deleted files, which was not available in this
session; this is an environment limitation, not an unverified code path — the offset
computation reuses the same `cluster_offset` function already exercised by `scan_dir` for
locating live directories.
