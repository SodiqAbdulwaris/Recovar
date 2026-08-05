#![cfg(windows)]
use super::{DiskReader, DriveInfo};
use anyhow::{bail, Result};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_NO_BUFFERING, FILE_FLAG_SEQUENTIAL_SCAN,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    // DRIVE_CDROM,
};
use windows_sys::Win32::System::WindowsProgramming::DRIVE_CDROM;
use windows_sys::Win32::System::IO::{DeviceIoControl, OVERLAPPED};
use windows_sys::Win32::System::Ioctl::{DISK_GEOMETRY_EX, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX};
use windows_sys::Win32::Storage::FileSystem::ReadFile;

pub struct WindowsDisk {
    handle: HANDLE,
    size: u64,
    sector_size: u32,
}

impl Drop for WindowsDisk {
    fn drop(&mut self) {
        unsafe { if self.handle != INVALID_HANDLE_VALUE { CloseHandle(self.handle); } }
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0u16)).collect()
}

pub fn open_drive(path: &str) -> Result<Box<dyn DiskReader>> {
    let raw_path = if path.len() >= 2 && path.chars().nth(1) == Some(':') {
        format!("\\\\.\\{}:", &path[..1])
    } else if path.starts_with("\\\\.\\" ) {
        path.to_string()
    } else {
        format!("\\\\.\\{}", path.trim_end_matches('\\'))
    };
    let wide = to_wide(&raw_path);
    let handle = unsafe {
        CreateFileW(wide.as_ptr(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(), OPEN_EXISTING, FILE_FLAG_NO_BUFFERING | FILE_FLAG_SEQUENTIAL_SCAN, 0)
    };
    if handle == INVALID_HANDLE_VALUE {
        let err = std::io::Error::last_os_error();
        bail!("Failed to open '{}': {}. Run as Administrator.", raw_path, err);
    }
    let mut geom = unsafe { std::mem::zeroed::<DISK_GEOMETRY_EX>() };
    let mut ret: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(handle, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
            std::ptr::null(), 0, &mut geom as *mut _ as *mut _,
            std::mem::size_of::<DISK_GEOMETRY_EX>() as u32, &mut ret, std::ptr::null_mut())
    };
    let (size, sector_size) = if ok != 0 {
        // (unsafe { geom.DiskSize.QuadPart } as u64, geom.Geometry.BytesPerSector)
        (geom.DiskSize as u64, geom.Geometry.BytesPerSector)
    } else { (0u64, 512u32) };
    Ok(Box::new(WindowsDisk { handle, size, sector_size }))
}

impl DiskReader for WindowsDisk {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let sector = self.sector_size as u64;
        let aligned_offset = (offset / sector) * sector;
        let delta = (offset - aligned_offset) as usize;
        let aligned_len = ((buf.len() + delta + sector as usize - 1) / sector as usize) * sector as usize;
        let mut aligned_buf = vec![0u8; aligned_len];
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.Anonymous.Anonymous.Offset = aligned_offset as u32;
        overlapped.Anonymous.Anonymous.OffsetHigh = (aligned_offset >> 32) as u32;
        let mut bytes_read: u32 = 0;
        let ok = unsafe {
            ReadFile(self.handle, aligned_buf.as_mut_ptr(), aligned_len as u32,
                &mut bytes_read, &mut overlapped)
        };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            return Err(anyhow::anyhow!("ReadFile failed at offset {}: {}", offset, err));
        }
        let available = (bytes_read as usize).saturating_sub(delta);
        let copy_len = buf.len().min(available);
        buf[..copy_len].copy_from_slice(&aligned_buf[delta..delta + copy_len]);
        Ok(copy_len)
    }
    fn size(&self) -> u64 { self.size }
    fn sector_size(&self) -> u32 { self.sector_size }
}

pub fn list_drives() -> Result<Vec<DriveInfo>> {
    let mask = unsafe { GetLogicalDrives() };
    let mut drives = Vec::new();
    for i in 0..26u32 {
        if mask & (1 << i) == 0 { continue; }
        let letter = (b'A' + i as u8) as char;
        let path = format!("{letter}:\\\\");
        let wide_path = to_wide(&path);
        let drive_type = unsafe { GetDriveTypeW(wide_path.as_ptr()) };
        if drive_type == DRIVE_CDROM { continue; }
        let removable = drive_type == 2; // DRIVE_REMOVABLE
        let mut label_buf = vec![0u16; 256];
        let mut fs_buf = vec![0u16; 256];
        let mut serial = 0u32;
        let mut max_comp = 0u32;
        let mut flags = 0u32;
        let vol_ok = unsafe {
            GetVolumeInformationW(wide_path.as_ptr(),
                label_buf.as_mut_ptr(), label_buf.len() as u32,
                &mut serial, &mut max_comp, &mut flags,
                fs_buf.as_mut_ptr(), fs_buf.len() as u32)
        };
        let label = if vol_ok != 0 {
            let end = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
            String::from_utf16_lossy(&label_buf[..end]).to_string()
        } else { format!("Drive {letter}:") };
        let filesystem = if vol_ok != 0 {
            let end = fs_buf.iter().position(|&c| c == 0).unwrap_or(0);
            String::from_utf16_lossy(&fs_buf[..end]).to_string()
        } else { "Unknown".to_string() };
        drives.push(DriveInfo {
            path: format!("{letter}:\\\\"),
            label: format!("{label} ({letter}:)"),
            size: 0,
            filesystem,
            removable,
        });
    }
    Ok(drives)
}
