# Fix: GUI binary crate could not link against the GUI library crate

## Problem

`recovar-gui/src-tauri/src/main.rs` called `recovar_gui_lib::run()`, but
`recovar-gui/src-tauri/src/lib.rs` defined a private `fn main()` instead of a public
`fn run()`. This is a mismatch from the standard Tauri scaffold (bin calls `lib::run()`).
The crate failed to compile with `error[E0425]: cannot find function 'run' in crate
'recovar_gui_lib'`. Combined with the missing icon (see the icon fix doc), the GUI had
apparently never successfully built.

## Approach

Renamed `fn main()` to `pub fn run()` in `lib.rs` so it matches what `main.rs` calls.

## Files affected

- `recovar-gui/src-tauri/src/lib.rs`

## Verification

`cargo build --release` succeeds and produces a working `recovar-gui.exe`.
