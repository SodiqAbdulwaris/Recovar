# Fix: Android recovery ignored the chosen output directory; GUI scan blocked stop_scan

## Problem

Antigravity's independent review (see below) and my own earlier review both flagged the same
issue: `RecoverySession::run_android` hard-coded its destination to `"./recovered_android"`,
ignoring the `--output` CLI flag and the GUI's output directory field entirely. Files were
pulled to a fixed relative path no matter what the user chose.

Antigravity also identified that `recovar-gui/src-tauri/src/commands.rs`'s `start_scan` calls
the blocking, potentially long-running `RecoverySession::run` directly inside an `async fn`
on Tauri's async executor. The `stop_scan` fix from earlier in this session wires up the stop
flag correctly, but if the executor thread running `start_scan` is the only one available,
`stop_scan` cannot actually be dispatched until the scan finishes anyway.

A third, related issue Antigravity caught: `save_file` treated `disk_offset == 0` as "skip
silently, report success" for every recovery method, including a legitimate case (ADB-pulled
files, which have no disk offset because they were already copied by `adb pull` during the
scan itself, not carved from a raw device). This made success and silent-no-op
indistinguishable.

## Approach

- `RecoverySession::run` now takes an `output_dir: &Path` parameter and threads it through to
  `run_android`, which uses it instead of the hard-coded path. `run_laptop` ignores it (a
  laptop scan doesn't write anything until a later `save_file` call).
- Updated both callers: `recovar-cli/src/main.rs` passes `&args.output`; the GUI's
  `start_scan` passes the user's `output_dir` field.
- `save_file` now special-cases `RecoveryMethod::AdbPull`: since that file is already on disk
  at `output_dir` from the scan, it returns the path directly instead of trying (and failing)
  to re-read it from a raw device. For every other recovery method, if `disk_offset == 0` or
  `size == 0` (meaning there is genuinely no data location to copy from), it now returns an
  `Err` instead of silently returning `Ok` without writing anything.
- `start_scan` now runs the actual scan inside `tauri::async_runtime::spawn_blocking`, keeping
  the async executor free to service `stop_scan` (and any other command) while a scan is in
  progress.
- In the GUI, `recover_files`' real returned count is now shown to the user
  (`"Saved N of M file(s)"`) instead of assuming every selected file succeeded.
- The drive/device `<select>` in `App.tsx` no longer refetches on every click; it only fetches
  when the list is still empty, removing a flicker Antigravity noticed on repeated clicks.

## Files affected

- `recovar-core/src/recovery.rs`
- `recovar-cli/src/main.rs`
- `recovar-gui/src-tauri/src/commands.rs`
- `recovar-gui/src/App.tsx`

## Verification

- `cargo test -p recovar-core` (7 tests, including the FAT32/NTFS/carver/adb simulation
  tests) still passes after the `save_file`/`run` signature changes.
- `cargo build --release` succeeds for the full workspace.
- Compiled a throwaway fake `adb.exe` (via `rustc`, same technique as the unit test) that
  answers one device with one file in `/sdcard/DCIM`, prepended it to `PATH`, and ran the real
  `recovar.exe scan --target android --mode quick --save --output <custom dir>`. Confirmed the
  file was written to the custom directory (not `./recovered_android`) with the exact expected
  bytes.
- `recovar.exe scan --drive G:\ --mode quick` and `recovar.exe scan --target android --mode
  quick` (no fake adb) both still fail gracefully with the expected errors (no Administrator,
  no device found respectively), confirming no regression in the laptop path.

## Multi-agent review

Antigravity (`agy`) was asked to independently review application behavior, GUI/core
integration boundaries, and runtime problems in `recovar-gui/src/App.tsx`,
`recovar-gui/src-tauri/src/commands.rs`, `recovar-gui/src-tauri/src/lib.rs`, and
`recovar-core/src/recovery.rs`. It reported 12 findings; the ones addressed here (blocking
scan thread, Android output directory, silent false-success in `save_file`, inaccurate
recovered-file count in the UI, dropdown refetch flicker) were verified against the code and
fixed. Lower-severity or more speculative findings not acted on in this pass: `save_file`
re-opening the raw drive handle once per file (a performance concern, not a correctness bug);
the redundant drive `<select>` + free-text `<input>` pairing; no explicit output-path
validation before writing. These are left as known, minor, low-risk items rather than
expanded scope.
