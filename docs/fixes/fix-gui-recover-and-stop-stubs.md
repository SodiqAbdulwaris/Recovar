# Fix: GUI "Recover Selected" and "Stop Scan" did nothing

## Problem

In `recovar-gui/src-tauri/src/commands.rs`:

- `recover_files` was a stub. It logged a message and returned
  `Ok(indices.len())` without ever calling `RecoverySession::save_file`. The GUI reported
  "Saved N file(s)" to the user while writing zero bytes to disk.
- `stop_scan` was a stub `async fn stop_scan() -> Result<(), String> { Ok(()) }`. It never
  touched `RecoverySession::stop_flag`, so the "Stop Scan" button had no effect on a running
  scan.
- `ActiveSession` (a `Mutex<Option<RecoverySession>>` intended to hold scan state) was
  declared but never constructed or registered with `.manage()`, so there was no way for a
  later command invocation to reach the session created by `start_scan`.

The CLI (`recovar-cli/src/main.rs`) already implements the correct pattern: keep the
`RecoverySession` around after scanning and call `session.save_file(...)` for each selected
result. The GUI never wired the equivalent.

## Approach

- Split `ActiveSession` into two mutexes: `stop_flag: Mutex<Option<Arc<AtomicBool>>>`, set
  *before* the (blocking) scan runs so a concurrent `stop_scan` call can reach it while the
  scan is still in progress; and `session: Mutex<Option<RecoverySession>>`, set once the scan
  completes so `recover_files` has something to save from.
- `start_scan` now stores the session's stop flag before calling `session.run(...)`, and
  stores the finished session afterward.
- `stop_scan` now loads the stop flag and sets it via `AtomicBool::store`, which the
  scan loops in `ntfs/mft.rs`, `fat/fat32.rs`, and `carver/engine.rs` already check on each
  iteration.
- `recover_files` now looks up the stored session, resolves the source drive from
  `session.target`, and calls `session.save_file(file, output_dir, &drive)` for each selected
  index, returning the real count of files actually written.
- Registered `ActiveSession::default()` with `.manage(...)` in `lib.rs` so all three commands
  share the same state.

## Files affected

- `recovar-gui/src-tauri/src/commands.rs`
- `recovar-gui/src-tauri/src/lib.rs`

## Verification

`cargo build --release` succeeds. Verified by code inspection that the scan loops
(`recovar-core/src/ntfs/mft.rs:58`, `recovar-core/src/fat/fat32.rs:72`,
`recovar-core/src/carver/engine.rs:30`) check the same `Arc<AtomicBool>` now shared with the
GUI's stop flag. End-to-end recovery from the GUI against a real disk was not exercised in
this session because it requires Administrator privileges, which were not available in this
environment; the CLI's `--save` path exercises the identical `save_file` code and was run
successfully (see `docs/testing/verify-cli-scan-and-list.md`).

## Notes

`start_scan` still runs the blocking scan directly inside an `async fn` rather than via
`tauri::async_runtime::spawn_blocking`. On the default multi-threaded Tokio runtime this does
not prevent `stop_scan` from being scheduled on another worker thread, but moving the scan to
`spawn_blocking` would be a more robust design if this becomes a problem in practice.
