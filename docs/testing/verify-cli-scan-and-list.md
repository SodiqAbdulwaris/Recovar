# Verification: build and CLI baseline

## What was run

- `cargo build --release` for the full workspace (`recovar-core`, `recovar-cli`,
  `recovar-gui/src-tauri`) — succeeded, exit code 0, after the fixes in `docs/fixes/`.
  Before those fixes the same command failed (missing icon, then a linker/name-resolution
  error in the GUI crate).
- `recovar.exe info` — prints platform info, Administrator status (correctly detected as
  "No" in this non-elevated session), ADB availability, and the two local drives.
- `recovar.exe list --target drives` — correctly lists `C:\` (NTFS) and `G:\` (FAT32) after
  the double-backslash fix.
- `recovar.exe scan --drive G:\ --mode quick` — without Administrator privileges, fails
  gracefully with a Windows `ReadFile` "Incorrect function" error and a printed warning to
  run as Administrator, instead of panicking. This matches the documented requirement in
  `setup.md` that raw disk access needs elevation.
- `npm run build` in `recovar-gui/` (tsc + vite) — succeeded after the CSS/JSX animation
  cleanup.
- Frontend rendering was checked by running `vite dev` and loading the page in a browser
  preview; the layout renders as expected and the browser console reported no errors. Tauri
  IPC commands (`list_drives`, `start_scan`, etc.) are not available outside the actual Tauri
  shell, so this only verifies markup/CSS, not the Rust command layer.

## What was not verified

Full end-to-end disk scanning and file recovery (both CLI `--save` and the GUI's "Recover
Selected") were not exercised against a live drive with real deleted files, because that
requires Administrator privileges not available in this session, and deliberately creating
test data on a real disk to carve back out was judged out of scope for an automated pass.
The `save_file` / offset-computation logic was verified by code review and by confirming it
reuses the same tested `cluster_offset` helper already exercised by the directory walker.

A small existing unit test suite in `recovar-core/src/carver/signatures.rs` (3 tests) could
not previously run at all: `cargo test -p recovar-core` failed to compile in isolation. See
`docs/fixes/fix-recovar-core-standalone-build.md`. A synthetic, disk-free recovery test suite
was added afterward; see `docs/testing/add-synthetic-recovery-tests.md`.
