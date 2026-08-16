use crate::types::{FileType, RecoveredFile, RecoveryMethod, ScanProgress};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone)]
pub struct AdbDevice {
    pub serial: String,
    pub model: String,
    pub state: String,
}

pub fn list_devices() -> Result<Vec<AdbDevice>> {
    let output = Command::new("adb").arg("devices").arg("-l")
        .output().context("Failed to run 'adb'. Install Android Platform Tools and add to PATH.")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    for line in stdout.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('*') { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }
        let serial = parts[0].to_string();
        let state = parts[1].to_string();
        let model = parts.iter().find(|p| p.starts_with("model:"))
            .map(|p| p.trim_start_matches("model:").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        devices.push(AdbDevice { serial, model, state });
    }
    Ok(devices)
}

pub fn pull_accessible_files(
    device: &AdbDevice,
    dest_dir: &Path,
    progress_cb: &mut dyn FnMut(ScanProgress),
    stop_flag: Arc<AtomicBool>,
) -> Result<Vec<RecoveredFile>> {
    std::fs::create_dir_all(dest_dir)?;
    let folders = ["/sdcard/DCIM", "/sdcard/Pictures", "/sdcard/Movies", "/sdcard/Download", "/sdcard/.Trash"];
    let mut results = Vec::new();
    for (fi, folder) in folders.iter().enumerate() {
        if stop_flag.load(Ordering::Relaxed) { break; }
        progress_cb(ScanProgress {
            bytes_scanned: fi as u64, bytes_total: folders.len() as u64, files_found: results.len(),
            phase: format!("ADB pull: scanning {folder}"), complete: false, warning: None,
        });
        let ls = match Command::new("adb").args(["-s", &device.serial, "shell", "find", folder, "-type", "f"]).output() {
            Ok(o) => o, Err(_) => continue,
        };
        for file_path in String::from_utf8_lossy(&ls.stdout).lines() {
            let file_path = file_path.trim();
            if file_path.is_empty() { continue; }
            let filename = Path::new(file_path).file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
            let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
            let file_type = match ext.as_str() {
                "jpg" | "jpeg" => FileType::Jpeg, "png" => FileType::Png,
                "mp4" => FileType::Mp4, "mov" => FileType::Mov, "avi" => FileType::Avi, "mkv" => FileType::Mkv,
                _ => continue,
            };
            let local_path = dest_dir.join(&filename);
            if Command::new("adb").args(["-s", &device.serial, "pull", file_path, local_path.to_str().unwrap_or(".")]).output().is_ok() {
                let size = local_path.metadata().map(|m| m.len()).unwrap_or(0);
                results.push(RecoveredFile {
                    name: Some(filename), original_path: Some(file_path.to_string()),
                    size, file_type, disk_offset: 0,
                    recovery_method: RecoveryMethod::AdbPull, confidence: 1.0, index: results.len(),
                });
            }
        }
    }
    progress_cb(ScanProgress {
        bytes_scanned: 1, bytes_total: 1, files_found: results.len(),
        phase: "ADB scan complete".to_string(), complete: true, warning: None,
    });
    Ok(results)
}

pub fn check_adb_available() -> bool {
    Command::new("adb").arg("version").stdout(Stdio::null()).stderr(Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fake `adb.exe` on PATH, compiled from a tiny Rust source at test
    // time with rustc (batch/script shims are unreliable here: Command's
    // Windows PATH search only auto-appends `.exe`, not the full PATHEXT
    // list, so a `.bat` stand-in silently loses to a real adb.exe elsewhere
    // on PATH). It answers `devices -l`, `shell find`, and `pull` the same
    // way real adb would for one fake device with two accessible files, so
    // pull_accessible_files can be exercised end-to-end without a real phone.
    const FAKE_ADB_SRC: &str = r#"
use std::{env, fs};
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let get = |i: usize| args.get(i).map(|s| s.as_str());
    if get(0) == Some("devices") {
        println!("List of devices attached");
        println!("EMULATOR123\tdevice model:Test_Device");
        return;
    }
    if get(0) == Some("-s") {
        match get(2) {
            Some("shell") => {
                match get(4) {
                    Some("/sdcard/DCIM") => println!("/sdcard/DCIM/photo1.jpg"),
                    Some("/sdcard/Movies") => println!("/sdcard/Movies/clip1.mp4"),
                    _ => {}
                }
                return;
            }
            Some("pull") => {
                fs::write(&args[4], format!("FAKE_CONTENT_FOR_{}", &args[3])).unwrap();
                return;
            }
            _ => {}
        }
    }
    std::process::exit(1);
}
"#;

    fn write_fake_adb(dir: &std::path::Path) {
        let src = dir.join("fake_adb.rs");
        std::fs::write(&src, FAKE_ADB_SRC).unwrap();
        let status = Command::new("rustc")
            .args(["--edition", "2021", "-o"])
            .arg(dir.join("adb.exe"))
            .arg(&src)
            .status()
            .expect("rustc must be available to build the fake adb binary");
        assert!(status.success(), "failed to compile fake adb.exe");
    }

    #[test]
    fn pulls_accessible_files_from_a_fake_device() {
        let tmp = std::env::temp_dir().join(format!("recovar_fake_adb_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        write_fake_adb(&tmp);

        let orig_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{};{}", tmp.display(), orig_path));
        let dest = tmp.join("recovered");

        let result = (|| -> Result<_> {
            let devices = list_devices()?;
            let stop = Arc::new(AtomicBool::new(false));
            let files = pull_accessible_files(&devices[0], &dest, &mut |_| {}, stop)?;
            Ok((devices, files))
        })();

        std::env::set_var("PATH", orig_path);
        let _ = std::fs::remove_dir_all(&tmp);
        let (devices, results) = result.expect("adb simulation should succeed");

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "EMULATOR123");
        assert_eq!(devices[0].model, "Test_Device");

        assert_eq!(results.len(), 2, "expected one photo and one video pulled");
        let names: Vec<String> = results.iter().filter_map(|f| f.name.clone()).collect();
        assert!(names.contains(&"photo1.jpg".to_string()));
        assert!(names.contains(&"clip1.mp4".to_string()));
        for f in &results {
            assert_eq!(f.recovery_method, RecoveryMethod::AdbPull);
            assert!(f.size > 0, "pulled file should have real bytes written to disk");
        }
    }
}
