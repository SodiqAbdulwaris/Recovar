# Fix: Tauri GUI failed to build (missing icon.ico)

## Problem

`cargo build --release` failed for `recovar-gui` with:

```
icons/icon.ico not found; required for generating a Windows Resource file during tauri-build
```

`tauri.conf.json` declared `"bundle": { "icon": [] }` and no `icons/` directory existed
anywhere in `recovar-gui/src-tauri`. `tauri-build` requires an `.ico` file to embed as the
Windows executable resource regardless of the bundle icon list, so the GUI could not be
built at all, on any machine.

## Approach

Generated a full Tauri icon set (`icon.ico`, `icon.icns`, and PNGs at standard sizes) from a
placeholder source image using `npx tauri icon`, since no source artwork exists in the repo.
Removed the iOS/Android icon variants the generator also produces, since this is a Windows
desktop app only. Pointed `tauri.conf.json`'s `bundle.icon` at the generated files.

## Files affected

- `recovar-gui/src-tauri/tauri.conf.json`
- `recovar-gui/src-tauri/icons/` (new: `icon.ico`, `icon.icns`, `32x32.png`, `64x64.png`,
  `128x128.png`, `128x128@2x.png`, `icon.png`, and Windows Store tile PNGs)

## Verification

`cargo build --release` for the full workspace completes with exit code 0 and produces
`target/release/recovar-gui.exe`.

## Notes

The generated icon is a plain placeholder (solid dark square), not real branding. Replace
`recovar-gui/src-tauri/icons/` with real artwork before shipping a public build; regenerate
with `npx tauri icon <source.png>` from `recovar-gui/`.
