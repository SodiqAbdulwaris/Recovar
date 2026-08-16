#![cfg(target_os = "linux")]
use super::{DiskReader, DriveInfo};
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;

// Linux block device ioctls (linux/fs.h). Numeric values are the standard,
// widely-used constants for these requests on all supported architectures.
const BLKGETSIZE64: libc::c_ulong = 0x8008_1272;
const BLKSSZGET: libc::c_ulong = 0x1268;

pub struct LinuxDisk {
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

    let mut size: u64 = 0;
    let size_ok = unsafe { libc::ioctl(fd, BLKGETSIZE64, &mut size as *mut u64) } == 0;
    let size = if size_ok { size } else { file.metadata().map(|m| m.len()).unwrap_or(0) };

    let mut sector_size: libc::c_int = 0;
    let ssz_ok = unsafe { libc::ioctl(fd, BLKSSZGET, &mut sector_size as *mut libc::c_int) } == 0;
    let sector_size = if ssz_ok && sector_size > 0 { sector_size as u32 } else { 512 };

    Ok(Box::new(LinuxDisk { file, size, sector_size }))
}

impl DiskReader for LinuxDisk {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        Ok(self.file.read_at(buf, offset)?)
    }
    fn size(&self) -> u64 { self.size }
    fn sector_size(&self) -> u32 { self.sector_size }
}

pub fn list_drives() -> Result<Vec<DriveInfo>> {
    let mut drives = Vec::new();
    let entries = match fs::read_dir("/sys/class/block") {
        Ok(e) => e,
        Err(_) => return Ok(drives),
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip loop devices, ramdisks, device-mapper (LUKS/LVM) and software
        // RAID members: none of these are meaningful raw recovery targets.
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("dm-") || name.starts_with("md") || name.starts_with("zram") {
            continue;
        }
        let sys_path = entry.path();
        let size_sectors: u64 = fs::read_to_string(sys_path.join("size"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        if size_sectors == 0 { continue; }
        let removable = fs::read_to_string(sys_path.join("removable"))
            .map(|s| s.trim() == "1")
            .unwrap_or(false);
        drives.push(DriveInfo {
            path: format!("/dev/{name}"),
            label: format!("/dev/{name}"),
            // Filesystem type is left "Unknown" here deliberately: the scan
            // itself detects NTFS/FAT32 by reading the volume's own boot
            // sector, the same way the Windows backend's callers do.
            size: size_sectors * 512,
            filesystem: "Unknown".to_string(),
            removable,
        });
    }
    drives.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(drives)
}
