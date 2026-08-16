# Fix: drive listing showed "C:\\" instead of "C:\"

## Problem

`recovar-core/src/disk/windows.rs`'s `list_drives()` built drive path strings with
`format!("{letter}:\\\\")`. In a Rust string literal `\\\\` is two escaped backslashes, so the
resulting string contained a literal double backslash: `C:\\` (4 characters) instead of the
intended `C:\` (3 characters). This was visible in both the CLI (`recovar list --target
drives` printed `C:\\`) and would have populated the GUI's drive dropdown with the same
malformed value.

The extra backslash happened to not break `open_drive` (which only inspects the first
character before the colon) or, apparently, the `GetVolumeInformationW`/`GetDriveTypeW` Win32
calls on the tested system, but it is incorrect and unprofessional-looking output, and is not
guaranteed to be tolerated by all Windows API surfaces.

## Approach

Changed both `format!("{letter}:\\\\")` occurrences to `format!("{letter}:\\")`, which
produces the correct single-backslash root path.

## Files affected

- `recovar-core/src/disk/windows.rs`

## Verification

`recovar.exe list --target drives` now prints `C:\` and `G:\` instead of `C:\\` and `G:\\`.
