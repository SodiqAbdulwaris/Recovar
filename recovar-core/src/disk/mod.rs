#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub mod stub;

use anyhow::Result;

#[derive(Debug, Clone)]
pub struct DriveInfo {
    pub path: String,
    pub label: String,
    pub size: u64,
    pub filesystem: String,
    pub removable: bool,
}

pub fn list_drives() -> Result<Vec<DriveInfo>> {
    #[cfg(windows)] { windows::list_drives() }
    #[cfg(target_os = "linux")] { linux::list_drives() }
    #[cfg(target_os = "macos")] { macos::list_drives() }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))] { stub::list_drives() }
}

pub fn open_drive(path: &str) -> Result<Box<dyn DiskReader>> {
    #[cfg(windows)] { windows::open_drive(path) }
    #[cfg(target_os = "linux")] { linux::open_drive(path) }
    #[cfg(target_os = "macos")] { macos::open_drive(path) }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))] { stub::open_drive(path) }
}

pub trait DiskReader: Send {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize>;
    fn size(&self) -> u64;
    fn sector_size(&self) -> u32;
}
