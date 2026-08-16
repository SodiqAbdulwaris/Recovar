# Fix: recovar-core failed to compile on its own (missing windows-sys feature)

## Problem

`cargo build --release` for the whole workspace succeeded, but `cargo test -p recovar-core`
(and `cargo build -p recovar-core` in isolation) failed with:

```
error[E0432]: unresolved import `windows_sys::Win32::Storage::FileSystem::CreateFileW`
```

`recovar-core/src/disk/windows.rs` calls `CreateFileW`, which in `windows-sys 0.52` is gated
behind two features: `Win32_Foundation` and `Win32_Security` (it takes a
`SECURITY_ATTRIBUTES` parameter). `recovar-core/Cargo.toml` only declared `Win32_Foundation`
and four other features, never `Win32_Security`.

This had been masked in the full-workspace release build: Cargo's resolver unifies feature
flags for a given dependency version across every crate in the build graph, and some other
workspace dependency (transitively, via the GUI's Tauri stack) also pulls in `windows-sys
0.52.0` and happens to request `Win32_Security` for its own purposes. Building the whole
workspace together let that borrowed feature slip into `recovar-core`'s compiled unit.
Building `recovar-core` alone — which is what `cargo test`, `cargo check -p recovar-core`, or
publishing the crate separately would do — exposed the real, incomplete dependency
declaration. This is also why the earlier `cargo test --workspace` run in this session showed
zero tests for `recovar-core`: the crate's own unit tests never actually got to run, and the
one place with real tests (`carver/signatures.rs`) was silently skipped.

## Approach

Added `"Win32_Security"` to `recovar-core/Cargo.toml`'s `windows-sys` feature list.

## Files affected

- `recovar-core/Cargo.toml`

## Verification

`cargo test -p recovar-core` now compiles and runs `recovar-core`'s three existing unit tests
(all in `carver/signatures.rs`) successfully in isolation, instead of failing to compile.
`cargo build --release` for the full workspace still succeeds as before.
