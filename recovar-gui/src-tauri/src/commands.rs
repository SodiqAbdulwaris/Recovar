use recovarcorelib::{
    android::adb::{self, AdbDevice},
    disk::{self, DriveInfo},
    types::{RecoveredFile, ScanMode, ScanProgress, Target},
    RecoverySession,
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, State, Emitter};

pub struct ActiveSession(pub Mutex<Option<RecoverySession>>);

#[tauri::command]
pub async fn list_drives() -> Result<Vec<DriveInfoDto>, String> {
    disk::list_drives()
        .map(|drives| drives.into_iter().map(DriveInfoDto::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_android_devices() -> Result<Vec<AdbDeviceDto>, String> {
    adb::list_devices()
        .map(|devs| devs.into_iter().map(AdbDeviceDto::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_scan(
    app: AppHandle,
    target: serde_json::Value,
    mode: String,
    output_dir: String,
) -> Result<Vec<RecoveredFile>, String> {
    let target_obj = target.as_object().ok_or("Invalid target")?;
    let target_core = if let Some(laptop) = target_obj.get("Laptop") {
        let drive = laptop["drive"].as_str().unwrap_or("C:\\").to_string();
        Target::Laptop { drive }
    } else if let Some(android) = target_obj.get("Android") {
        let serial = android["serial"].as_str().map(|s| s.to_string());
        let image_path = android["image_path"].as_str().map(|s| s.to_string());
        Target::Android { serial, image_path }
    } else {
        return Err("Unknown target type".to_string());
    };

    let scan_mode = match mode.as_str() {
        "quick" => ScanMode::QuickScan,
        "deep"  => ScanMode::DeepScan,
        _       => ScanMode::Both,
    };

    let app_clone = app.clone();
    let mut session = RecoverySession::new(target_core, scan_mode);

    let results = session.run(&mut move |progress: ScanProgress| {
        let _ = app_clone.emit("scan-progress", &progress);
    });

    match results {
        Ok(files) => {
            for file in files.iter() {
                let _ = app.emit("file-found", file);
            }
            Ok(files.clone())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn stop_scan() -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn recover_files(
    indices: Vec<usize>,
    output_dir: String,
) -> Result<usize, String> {
    tracing::info!("Recover {} files to {}", indices.len(), output_dir);
    Ok(indices.len())
}

// ---- DTOs ----

#[derive(Serialize, Deserialize, Clone)]
pub struct DriveInfoDto {
    pub path: String,
    pub label: String,
    pub size: u64,
    pub filesystem: String,
    pub removable: bool,
}

impl From<DriveInfo> for DriveInfoDto {
    fn from(d: DriveInfo) -> Self {
        Self {
            path: d.path,
            label: d.label,
            size: d.size,
            filesystem: d.filesystem,
            removable: d.removable,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AdbDeviceDto {
    pub serial: String,
    pub model: String,
    pub state: String,
}

impl From<AdbDevice> for AdbDeviceDto {
    fn from(d: AdbDevice) -> Self {
        Self {
            serial: d.serial,
            model: d.model,
            state: d.state,
        }
    }
}
