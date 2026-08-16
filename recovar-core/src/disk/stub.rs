#![cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
use super::{DiskReader, DriveInfo};
use anyhow::{bail, Result};
pub fn open_drive(_path: &str) -> Result<Box<dyn DiskReader>> {
    bail!("Raw disk access is not implemented on this platform.")
}
pub fn list_drives() -> Result<Vec<DriveInfo>> { Ok(vec![]) }
