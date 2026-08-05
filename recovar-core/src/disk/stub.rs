#![cfg(not(windows))]
use super::{DiskReader, DriveInfo};
use anyhow::{bail, Result};
pub fn open_drive(_path: &str) -> Result<Box<dyn DiskReader>> {
    bail!("Raw disk access is only supported on Windows.")
}
pub fn list_drives() -> Result<Vec<DriveInfo>> { Ok(vec![]) }
