#![cfg(target_os = "macos")]
use super::{DiskReader, DriveInfo};
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;

// IOKit storage ioctls (IOKit/storage/IOMediaBSDClient.h):
// DKIOCGETBLOCKSIZE = _IOR('d', 24, uint32_t), DKIOCGETBLOCKCOUNT = _IOR('d', 25, uint64_t).
const DKIOCGETBLOCKSIZE: libc::c_ulong = 0x4004_6418;
const DKIOCGETBLOCKCOUNT: libc::c_ulong = 0x4008_6419;

pub struct MacDisk {
    file: File,
    size: u64,
    sector_size: u32,
}

fn normalize_path(path: &str) -> String {
    if path.starts_with("/dev/") {
        path.to_string()
    } else {
        format!("/dev/{}", path.trim_start_matches('/'))
    }
}

pub fn open_drive(path: &str) -> Result<Box<dyn DiskReader>> {
    let dev_path = normalize_path(path);
    let file = File::open(&dev_path)
        .with_context(|| format!("Cannot open '{dev_path}'. Run with sudo for raw disk access."))?;
    let fd = file.as_raw_fd();

    let mut sector_size: u32 = 0;
    let ssz_ok = unsafe { libc::ioctl(fd, DKIOCGETBLOCKSIZE, &mut sector_size as *mut u32) } == 0;
    let sector_size = if ssz_ok && sector_size > 0 { sector_size } else { 512 };

    let mut block_count: u64 = 0;
    let count_ok = unsafe { libc::ioctl(fd, DKIOCGETBLOCKCOUNT, &mut block_count as *mut u64) } == 0;
    let size = if count_ok { block_count * sector_size as u64 } else { 0 };

    Ok(Box::new(MacDisk { file, size, sector_size }))
}

impl DiskReader for MacDisk {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        Ok(self.file.read_at(buf, offset)?)
    }
    fn size(&self) -> u64 { self.size }
    fn sector_size(&self) -> u32 { self.sector_size }
}

pub fn list_drives() -> Result<Vec<DriveInfo>> {
    let mut drives = Vec::new();
    let entries = match fs::read_dir("/dev") {
        Ok(e) => e,
        Err(_) => return Ok(drives),
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Whole disks (diskN) and slices (diskNsM); skip the unbuffered
        // "rdiskN" raw-device duplicates so each volume is listed once.
        if !name.starts_with("disk") { continue; }
        let rest = &name[4..];
        if rest.is_empty() || !rest.chars().next().unwrap().is_ascii_digit() { continue; }

        let dev_path = format!("/dev/{name}");
        let size = File::open(&dev_path).ok().and_then(|f| {
            let fd = f.as_raw_fd();
            let mut sector_size: u32 = 512;
            unsafe { libc::ioctl(fd, DKIOCGETBLOCKSIZE, &mut sector_size as *mut u32) };
            let mut block_count: u64 = 0;
            let ok = unsafe { libc::ioctl(fd, DKIOCGETBLOCKCOUNT, &mut block_count as *mut u64) } == 0;
            ok.then(|| block_count * sector_size as u64)
        }).unwrap_or(0);
        if size == 0 { continue; }

        drives.push(DriveInfo {
            path: dev_path.clone(),
            label: dev_path,
            // Filesystem type is left "Unknown": the scan itself detects
            // NTFS/FAT32 by reading the volume's own boot sector.
            size,
            removable: false, // not determinable without an IOKit device tree walk
            filesystem: "Unknown".to_string(),
        });
    }
    drives.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(drives)
}
