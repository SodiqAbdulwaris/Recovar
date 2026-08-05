use crate::android::adb;
use crate::carver::carve;
use crate::disk;
use crate::fat::scan_fat32;
use crate::ntfs::scan_ntfs;
use crate::types::{RecoveredFile, ScanMode, ScanProgress, Target};
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct RecoverySession {
    pub target: Target,
    pub mode: ScanMode,
    pub results: Vec<RecoveredFile>,
    pub stop_flag: Arc<AtomicBool>,
}

impl RecoverySession {
    pub fn new(target: Target, mode: ScanMode) -> Self {
        Self { target, mode, results: Vec::new(), stop_flag: Arc::new(AtomicBool::new(false)) }
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn run(&mut self, progress_cb: &mut dyn FnMut(ScanProgress)) -> Result<&Vec<RecoveredFile>> {
        match &self.target.clone() {
            Target::Laptop { drive } => self.run_laptop(drive, progress_cb)?,
            Target::Android { serial, image_path } => self.run_android(serial.clone(), image_path.clone(), progress_cb)?,
        }
        Ok(&self.results)
    }

    fn run_laptop(&mut self, drive: &str, progress_cb: &mut dyn FnMut(ScanProgress)) -> Result<()> {
        let mut reader = disk::open_drive(drive)
            .with_context(|| format!("Cannot open drive '{}'. Run as Administrator.", drive))?;
        let mut boot = [0u8; 512];
        reader.read_at(0, &mut boot)?;
        let is_ntfs = &boot[3..11] == b"NTFS    ";
        let is_fat32 = boot[510] == 0x55 && boot[511] == 0xAA;
        let mut combined = Vec::new();
        if matches!(self.mode, ScanMode::QuickScan | ScanMode::Both) {
            progress_cb(ScanProgress {
                bytes_scanned: 0, bytes_total: reader.size(), files_found: 0,
                phase: "Starting quick scan...".to_string(), complete: false, warning: None,
            });
            if is_ntfs {
                match scan_ntfs(reader.as_mut(), progress_cb, self.stop_flag.clone()) {
                    Ok(mut found) => combined.append(&mut found),
                    Err(e) => tracing::warn!("NTFS scan error: {}", e),
                }
            } else if is_fat32 {
                match scan_fat32(reader.as_mut(), progress_cb, self.stop_flag.clone()) {
                    Ok(mut found) => combined.append(&mut found),
                    Err(e) => tracing::warn!("FAT32 scan error: {}", e),
                }
            } else {
                progress_cb(ScanProgress {
                    bytes_scanned: 0, bytes_total: reader.size(), files_found: 0,
                    phase: "Unknown filesystem, skipping quick scan".to_string(), complete: false,
                    warning: Some("Drive is not NTFS or FAT32; quick scan skipped.".to_string()),
                });
            }
        }
        if matches!(self.mode, ScanMode::DeepScan | ScanMode::Both) {
            match carve(reader.as_mut(), 0, progress_cb, self.stop_flag.clone()) {
                Ok(mut found) => combined.append(&mut found),
                Err(e) => tracing::warn!("Carver error: {}", e),
            }
        }
        for (i, f) in combined.iter_mut().enumerate() { f.index = i; }
        self.results = combined;
        Ok(())
    }

    fn run_android(&mut self, serial: Option<String>, _image_path: Option<String>, progress_cb: &mut dyn FnMut(ScanProgress)) -> Result<()> {
        let devices = adb::list_devices()?;
        let device = match &serial {
            Some(s) => devices.iter().find(|d| &d.serial == s),
            None => devices.first(),
        }.ok_or_else(|| anyhow::anyhow!("No Android device found. Connect your phone and enable USB Debugging."))?;
        let dest = Path::new("./recovered_android");
        let found = adb::pull_accessible_files(device, dest, progress_cb, self.stop_flag.clone())?;
        self.results = found;
        Ok(())
    }

    pub fn save_file(&self, file: &RecoveredFile, output_dir: &Path, drive: &str) -> Result<std::path::PathBuf> {
        std::fs::create_dir_all(output_dir)?;
        let filename = file.name.clone().unwrap_or_else(|| {
            format!("recovered_{:06}.{}", file.index, file.file_type.extension())
        });
        let out_path = output_dir.join(&filename);
        if file.disk_offset > 0 && file.size > 0 {
            let mut reader = disk::open_drive(drive)?;
            let mut data = vec![0u8; file.size as usize];
            reader.read_at(file.disk_offset, &mut data)?;
            std::fs::write(&out_path, &data)?;
        }
        Ok(out_path)
    }
}
