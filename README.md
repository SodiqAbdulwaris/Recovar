# Recovar — File Recovery Tool

> Recover deleted files from your Windows, Linux, or macOS computer, and pull accessible files off an Android phone.

[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

## Features

- **NTFS Quick Scan** — Parses the Master File Table ($MFT) to find deleted file records with original filenames
- **FAT32 Quick Scan** — Scans FAT32 directory entries for the 0xE5 deleted marker
- **Deep Scan / Disk Carving** — Signature-based raw recovery for JPEG, PNG, MP4, MOV, AVI, MKV, PDF, DOCX and more
- **Android** — Pulls files still present in accessible storage (camera roll, downloads, and the on-device trash) via ADB, no root required. There is no raw partition carving on Android: recovering files permanently deleted from the device's own storage would require root and filesystem-specific (ext4/F2FS) carving, which is not implemented.
- **Dual Interface** — Full-featured CLI and Tauri GUI, responsive down to a folded-phone-width window

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
│   └── disk/        # Platform disk abstraction (Windows, Linux, macOS)
├── recovar-cli/     # CLI binary
└── recovar-gui/     # Tauri 2.0 desktop GUI
```

## Important Notes

- **Administrator privileges** (Windows) or **root/sudo** (Linux, macOS) required for raw disk access (laptop mode)
- **USB Debugging** must be enabled on Android
- Android recovery only pulls files that still exist somewhere accessible (camera roll, downloads, on-device trash) — it does not recover files permanently deleted from Android's own storage; see the Android note under Features
- Stop using the device immediately after data loss
- The Linux and macOS disk backends are new and have not been run against real hardware yet — they type-check and are structurally identical to the Windows backend, but treat them as less battle-tested until you've verified them yourself

## License

MIT
