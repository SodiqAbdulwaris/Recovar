use recovarcorelib::{
    android::adb::{self, AdbDevice},
    disk::{self, DriveInfo},
    types::{RecoveredFile, ScanMode, ScanProgress, Target},
    RecoverySession,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State, Emitter};

#[derive(Default)]
pub struct ActiveSession {
    // Set before the (blocking) scan runs so stop_scan can reach a scan in progress.
    pub stop_flag: Mutex<Option<Arc<AtomicBool>>>,
    // Set once the scan completes, so recover_files can save from its results.
    pub session: Mutex<Option<RecoverySession>>,
}

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
    state: State<'_, ActiveSession>,
    target: serde_json::Value,
    mode: String,
    #[allow(unused_variables)] output_dir: String,
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
    *state.stop_flag.lock().unwrap() = Some(session.stop_flag.clone());

    let results = session.run(&mut move |progress: ScanProgress| {
        let _ = app_clone.emit("scan-progress", &progress);
    });

    *state.stop_flag.lock().unwrap() = None;

    match results {
        Ok(files) => {
            let files = files.clone();
            for file in &files {
                let _ = app.emit("file-found", file);
            }
            *state.session.lock().unwrap() = Some(session);
            Ok(files)
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn stop_scan(state: State<'_, ActiveSession>) -> Result<(), String> {
    if let Some(flag) = state.stop_flag.lock().unwrap().as_ref() {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn recover_files(
    state: State<'_, ActiveSession>,
    indices: Vec<usize>,
    output_dir: String,
) -> Result<usize, String> {
    let guard = state.session.lock().unwrap();
    let session = guard.as_ref().ok_or("No completed scan to recover from")?;
    let drive = match &session.target {
        Target::Laptop { drive } => drive.clone(),
        Target::Android { .. } => String::new(),
    };
    let out_dir = Path::new(&output_dir);
    let mut saved = 0;
    for file in session.results.iter().filter(|f| indices.contains(&f.index)) {
        match session.save_file(file, out_dir, &drive) {
            Ok(_) => saved += 1,
            Err(e) => tracing::warn!("Failed to save file #{}: {}", file.index, e),
        }
    }
    Ok(saved)
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
