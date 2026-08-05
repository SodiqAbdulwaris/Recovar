# Recovar — File Recovery Tool

> Recover deleted files from your Windows laptop and Android phone.

[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

## Features

- **NTFS Quick Scan** — Parses the Master File Table ($MFT) to find deleted file records with original filenames
- **FAT32 Quick Scan** — Scans FAT32 directory entries for the 0xE5 deleted marker
- **Deep Scan / Disk Carving** — Signature-based raw recovery for JPEG, PNG, MP4, MOV, AVI, MKV, PDF, DOCX and more
- **Android Support** — Pull accessible files via ADB (no root); raw partition carving (root required)
- **Dual Interface** — Full-featured CLI and modern Tauri GUI

## Supported File Types

| Category | Types |
|---|---|
| Images | JPEG, PNG, GIF, BMP |
| Videos | MP4, MOV, AVI, MKV |
| Documents | PDF, DOCX, ZIP |
| Audio | MP3 |

## Quick Start

```powershell
# List available drives
.\target\release\recovar.exe list --target drives

# Scan D:\ drive (both quick and deep) and list results
.\target\release\recovar.exe scan --drive D:\ --mode both

# Save recovered images/videos to ./recovered
.\target\release\recovar.exe scan --drive D:\ --mode deep --save --output .\recovered

# Android: scan connected phone
.\target\release\recovar.exe scan --target android --mode quick

# List connected Android devices
.\target\release\recovar.exe list --target devices
```

## Building

See [setup.md](setup.md) for full instructions.

```powershell
cargo build --release -p recovar-cli
```

## Architecture

```
Recovar/
├── recovar-core/    # Core engine (shared library)
│   ├── ntfs/        # NTFS MFT parser
│   ├── fat/         # FAT32 directory scanner
│   ├── carver/      # Raw signature-based carving
│   ├── android/     # ADB bridge
│   └── disk/        # Platform disk abstraction (Windows)
├── recovar-cli/     # CLI binary
└── recovar-gui/     # Tauri 2.0 desktop GUI
```

## Important Notes

- **Administrator privileges** required for raw disk access (laptop mode)
- **USB Debugging** must be enabled on Android
- **Root access** required for Android deep scan
- Stop using the device immediately after data loss

## License

MIT
