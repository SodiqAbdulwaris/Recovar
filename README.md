# Recovar — File Recovery Tool

> Recover deleted files from your Windows, Linux, or macOS computer, and pull accessible files off an Android phone.

[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

Recovar reads raw disk sectors directly to find files that have been deleted but not yet
overwritten, and carves file signatures out of raw bytes when filesystem metadata is gone
entirely. It ships as both a scriptable CLI and a desktop GUI, sharing one recovery engine.

## Contents

- [What it can and can't recover](#what-it-can-and-cant-recover)
- [Supported file types](#supported-file-types)
- [Platform support](#platform-support)
- [Quick start (CLI)](#quick-start-cli)
- [Using the GUI](#using-the-gui)
- [Building from source](#building-from-source)
- [How recovery works](#how-recovery-works)
- [Architecture](#architecture)
- [Testing](#testing)
- [Important safety notes](#important-safety-notes)
- [Documentation](#documentation)
- [License](#license)

## What it can and can't recover

**Laptop / desktop (Windows, Linux, macOS):**

| Mode | How it works | Recovers |
|---|---|---|
| Quick scan (NTFS) | Reads the Master File Table ($MFT) for records still flagged as deleted | Original filename, size, and file bytes, if the sectors haven't been overwritten |
| Quick scan (FAT32) | Reads directory entries for the `0xE5` deleted marker | Same as above, for FAT32 volumes (most USB drives and SD cards) |
| Deep scan (carving) | Scans every raw sector for known file signatures, independent of filesystem metadata | File bytes with no original filename, even after the volume has been formatted |

The hard limit on all three: if the physical sectors have been overwritten since deletion —
new files written, the drive used heavily, or an SSD's TRIM having reclaimed the blocks — the
data is genuinely gone. No software can recover it. **Stop using the affected drive
immediately** to maximize what's still recoverable; see
[Important safety notes](#important-safety-notes).

**Android:** Recovar pulls files that are **still physically present** somewhere accessible —
`/sdcard/DCIM`, `/Pictures`, `/Movies`, `/Download`, and `/.Trash` (the on-device recycle bin,
if your gallery or file manager has one; retention is typically ~30 days and varies by
device). This is a backup/copy operation, not undelete. **There is no raw partition carving
or root-based recovery implemented for Android** — recovering a file permanently deleted from
Android's own storage would require root access plus filesystem-specific (ext4/F2FS) carving,
neither of which exists in this codebase today. If you deleted a photo two weeks ago and it's
no longer in your gallery's trash, Recovar as it exists today cannot get it back.

## Supported file types

| Category | Types |
|---|---|
| Images | JPEG, PNG, GIF, BMP |
| Videos | MP4, MOV, AVI, MKV |
| Documents | PDF, DOCX, ZIP |
| Audio | MP3 |

## Platform support

| Platform | Laptop recovery | Notes |
|---|---|---|
| Windows | ✅ | Requires Administrator privileges |
| Linux | ✅ | Requires `sudo`/root. Newer backend; verified by cross-target type checking, not yet run against real hardware |
| macOS | ✅ | Requires `sudo`/root. Same caveat as Linux |
| Android (as a scan target, from any of the above) | ✅ (accessible files only) | Requires ADB and USB debugging; see limitations above |

## Quick start (CLI)

```powershell
# Windows (run as Administrator for drive scans)
.\target\release\recovar.exe list --target drives
.\target\release\recovar.exe scan --drive D:\ --mode both
.\target\release\recovar.exe scan --drive D:\ --mode deep --save --output .\recovered
```

```bash
# Linux / macOS (run with sudo for drive scans)
sudo ./target/release/recovar list --target drives
sudo ./target/release/recovar scan --drive /dev/sdb1 --mode both
sudo ./target/release/recovar scan --drive /dev/sdb1 --mode deep --save --output ./recovered
```

```bash
# Android (any platform, no elevation needed — uses ADB)
recovar list --target devices
recovar scan --target android --mode quick --save --output ./recovered_android
```

Other useful flags: `--filter jpg,png` to limit results to specific types, `--limit 50` to cap
how many results are shown, `-v` for verbose logging. Run `recovar --help` or
`recovar scan --help` for the full list.

## Using the GUI

```bash
cd recovar-gui
npm install
npm run tauri dev
```

The GUI mirrors the CLI's workflow: pick a target (this computer or an Android device), pick
a scan depth, start the scan, and results are grouped into "High confidence" (recommended)
and "Needs review" (partial data, verify before relying on them). Select files and use
**Recover selected** to save them. The window is responsive from a full desktop layout down
to a folded-phone-width window.

## Building from source

See [setup.md](setup.md) for a full walkthrough (Windows-focused, including ADB platform
tools setup). In short:

```bash
# Prerequisites: Rust (rustup.rs), Node.js 18+ for the GUI
cargo build --release -p recovar-cli      # CLI only
cargo build --release                     # CLI + GUI (needs Node deps installed in recovar-gui/ first)
```

The CLI binary lands at `target/release/recovar` (`recovar.exe` on Windows); the GUI binary at
`target/release/recovar-gui`.

## How recovery works

Recovar never modifies the drive it's reading. It opens the raw device (`\\.\D:` on Windows,
`/dev/sdX` on Linux, `/dev/diskN` on macOS) read-only and either:

1. **Follows filesystem metadata that's still intact** — the NTFS $MFT or FAT32 directory
   entries record where deleted files' data used to start, even after the entry itself is
   marked deleted. This is fast and recovers original filenames.
2. **Or scans raw bytes for file signatures** when metadata isn't usable — every supported
   file type has a distinctive header (and often a footer) that can be found directly in the
   sector data, independent of any filesystem. This is slower but works even on formatted
   drives.

Each recovered file gets a **confidence score** based on how it was found: filesystem-metadata
recoveries with intact size information score highest; carved files where a footer/end marker
was found score high; carved files with no footer (so the true end of the file is a guess)
score lower and are flagged for manual review.

## Architecture

```
Recovar/
├── recovar-core/    # Core engine (shared library)
│   ├── ntfs/        # NTFS MFT parser
│   ├── fat/         # FAT32 directory scanner
│   ├── carver/      # Raw signature-based carving
│   ├── android/     # ADB bridge
│   └── disk/        # Platform disk abstraction (Windows, Linux, macOS)
├── recovar-cli/     # CLI binary
└── recovar-gui/     # Tauri 2.0 desktop GUI (React/TypeScript frontend, Rust backend)
```

`recovar-core` has no knowledge of the CLI or GUI — both are thin frontends over the same
`RecoverySession` API (scan, then selectively save recovered files). See
[docs/decisions/keep-tauri-not-slint.md](docs/decisions/keep-tauri-not-slint.md) for why the
GUI stays on Tauri rather than a native Rust UI toolkit.

## Testing

```bash
cargo test -p recovar-core
```

`recovar-core` has unit tests for all four recovery code paths (NTFS, FAT32, signature
carving, Android ADB pull) that build real byte-level fixtures — a synthetic FAT32/NTFS image,
an embedded JPEG signature, a simulated ADB device — and assert the recovered bytes match the
original data, not just that the code compiles. See
[docs/testing/add-synthetic-recovery-tests.md](docs/testing/add-synthetic-recovery-tests.md).

## Important safety notes

- **Stop using the affected drive or device immediately.** Every file you save, every
  application you open, every temp file the OS writes can overwrite the sectors holding a
  file you're trying to recover. Run Recovar from a different drive if possible, and save
  recovered files to a different drive than the one being scanned.
- **Administrator privileges** (Windows) or **root/sudo** (Linux, macOS) are required for raw
  disk access. Android recovery via ADB does not need elevated privileges.
- **USB debugging** must be enabled on the Android device, and you must approve the "Allow
  USB debugging?" prompt on the phone itself.
- The Linux and macOS disk backends are new: they've been verified with full cross-target
  type checking (including every `unsafe` ioctl call) but not yet run against real hardware.
  Treat them as implemented, not yet field-tested.

## Documentation

Meaningful engineering changes, UI redesigns, and architectural decisions are documented
individually under [`docs/`](docs/), organized by category (`fixes/`, `ui/`, `testing/`,
`architecture/`, `decisions/`). Start there for the reasoning behind any non-obvious part of
the codebase.

## License

MIT
