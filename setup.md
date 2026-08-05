# Recovar — Setup Guide

This guide walks you through everything needed to build and use Recovar on your Windows PC to recover deleted files from your **Windows laptop** or **Android phone**.

---

## Table of Contents
1. [Prerequisites](#1-prerequisites)
2. [Building Recovar](#2-building-recovar)
3. [Laptop Setup (Windows NTFS/FAT32)](#3-laptop-setup-windows-ntfsfat32)
4. [Android Phone Setup](#4-android-phone-setup)
5. [Running the CLI](#5-running-the-cli)
6. [Running the GUI](#6-running-the-gui)
7. [Troubleshooting](#7-troubleshooting)

---

## 1. Prerequisites

### 1.1 Rust Toolchain

Recovar is written in Rust. Install the Rust toolchain:

1. Go to [https://rustup.rs](https://rustup.rs)
2. Download and run `rustup-init.exe`
3. Choose the default installation (press Enter)
4. After installation, open a **new** PowerShell window and verify:

```powershell
rustc --version
cargo --version
```

You should see something like `rustc 1.78.0` or later.

### 1.2 ADB Platform Tools (for Android recovery)

Android Debug Bridge (ADB) is required for Android phone recovery.

1. Download **Android Platform Tools** from Google:
   👉 https://developer.android.com/tools/releases/platform-tools
2. Extract the ZIP file to a folder (e.g., `C:\adb\`)
3. Add the folder to your system PATH:
   - Open **Start** → search "Environment Variables"
   - Click "Edit the system environment variables"
   - Click "Environment Variables"
   - Under "System variables", find **Path** → click **Edit**
   - Click **New** → enter `C:\adb\platform-tools`
   - Click OK on all dialogs
4. Open a new PowerShell window and verify:

```powershell
adb version
```

You should see `Android Debug Bridge version 1.x.x`.

### 1.3 GUI Prerequisites (for Tauri GUI only)

The GUI is built with Tauri 2.0 and requires:

- **Node.js** (v18 or later): https://nodejs.org/
- **WebView2** (usually pre-installed on Windows 10/11)
  - If missing: https://developer.microsoft.com/en-us/microsoft-edge/webview2/

```powershell
node --version   # should be v18+
npm --version
```

---

## 2. Building Recovar

### 2.1 Clone or Open the Project

Navigate to the project folder:

```powershell
cd "C:\Users\HP\S.A Stuffs\MyWorks\Projects\Recovar"
```

### 2.2 Build the CLI (Required)

```powershell
cargo build --release -p recovar-cli
```

The binary will be at:
```
target\release\recovar.exe
```

---

## 3. Laptop Setup (Windows NTFS/FAT32)

### 3.1 Why Administrator is Required

Recovar accesses **raw disk sectors** directly, bypassing the Windows filesystem. This is the same level of access professional forensics tools use. Windows restricts raw disk access to Administrator accounts for security.

### 3.2 Running as Administrator

Open PowerShell as Administrator, then:
```powershell
# Run the scanner
.\target\release\recovar.exe scan --drive D:\ --mode both
```

### 3.3 Find Your Drive Letter

```powershell
# List all available drives
.\target\release\recovar.exe list --target drives
```

Or check in **File Explorer** or **Disk Management** (`Win + R` → `diskmgmt.msc`).

---

## 4. Android Phone Setup

### 4.1 Enable Developer Mode

1. Open **Settings** on your phone
2. Go to **About phone**
3. Tap **Build number** 7 times rapidly
4. You'll see "You are now a developer!"

### 4.2 Enable USB Debugging

1. Go to **Settings** → **Developer options** (now visible)
2. Scroll down and toggle **USB Debugging** to ON
3. Confirm the prompt

### 4.3 Connect Your Phone

1. Connect your phone to your PC with a USB cable
2. On your phone, a prompt will appear: **\"Allow USB Debugging?\"**
3. Tap **Always allow from this computer**, then **OK**
4. Verify connection:

```powershell
adb devices
```

You should see your device listed as `device` (not `unauthorized`):
```
List of devices attached
ABCDEF123456   device   model:Samsung_A52
```

### 4.4 What Can Be Recovered Without Root

Without root access, Recovar can scan and pull:
- Files in `/sdcard/DCIM` (camera photos and videos)
- Files in `/sdcard/Pictures`
- Files in `/sdcard/Movies`
- Files in `/sdcard/Download`
- Files in `/sdcard/.Trash` (recently deleted files)

---

## 5. Running the CLI

> Always run from an **Administrator** PowerShell for laptop recovery.

```powershell
# List drives
.\target\release\recovar.exe list --target drives

# LAPTOP: Quick scan C: drive (fast, needs NTFS/FAT32 metadata)
.\target\release\recovar.exe scan --drive C:\ --mode quick

# LAPTOP: Deep scan D: drive (slower, carves raw sectors)
.\target\release\recovar.exe scan --drive D:\ --mode deep --save --output E:\recovered

# ANDROID: Accessible files (no root)
.\target\release\recovar.exe scan --target android --mode quick --save --output .\recovered_android
```

---

## 6. Running the GUI

The GUI is being developed using Tauri 2.0. Run in dev mode:

```powershell
cd recovar-gui
npm run tauri dev
```

---

## 7. Troubleshooting

### "Access is denied" on disk scan
**Cause:** Not running as Administrator.  
**Fix:** Right-click PowerShell → "Run as administrator", then re-run the command.

### "adb: command not found"
**Cause:** ADB Platform Tools not on PATH.  
**Fix:** Add `C:\adb\platform-tools` (or wherever platform tools are extracted) to your PATH.
