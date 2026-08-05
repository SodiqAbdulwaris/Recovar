#[cfg(windows)]
pub mod windows;
#[cfg(not(windows))]
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
    #[cfg(not(windows))] { stub::list_drives() }
}

pub fn open_drive(path: &str) -> Result<Box<dyn DiskReader>> {
    #[cfg(windows)] { windows::open_drive(path) }
    #[cfg(not(windows))] { stub::open_drive(path) }
}

pub trait DiskReader: Send {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize>;
    fn size(&self) -> u64;
    fn sector_size(&self) -> u32;
}
