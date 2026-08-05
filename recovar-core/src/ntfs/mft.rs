use crate::disk::DiskReader;
use crate::types::{FileType, RecoveredFile, RecoveryMethod, ScanProgress};
use anyhow::{bail, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const MFT_RECORD_SIZE: usize = 1024;
const NTFS_SIGNATURE: &[u8] = b"NTFS    ";
const MFT_RECORD_SIG: &[u8] = b"FILE";

struct NtfsBoot {
    bytes_per_cluster: u64,
    mft_offset: u64,
}

pub fn scan_ntfs(
    reader: &mut dyn DiskReader,
    progress_cb: &mut dyn FnMut(ScanProgress),
    stop_flag: Arc<AtomicBool>,
) -> Result<Vec<RecoveredFile>> {
    let mut boot_buf = [0u8; 512];
    reader.read_at(0, &mut boot_buf)?;
    if &boot_buf[3..11] != NTFS_SIGNATURE {
        bail!("Not an NTFS volume.");
    }
    let boot = parse_boot(&boot_buf)?;
    progress_cb(ScanProgress {
        bytes_scanned: 0, bytes_total: reader.size(), files_found: 0,
        phase: "Quick scan: reading NTFS MFT".to_string(), complete: false, warning: None,
    });
    scan_mft(reader, &boot, progress_cb, stop_flag)
}

fn parse_boot(buf: &[u8]) -> Result<NtfsBoot> {
    let mut c = Cursor::new(buf);
    c.set_position(0x0B);
    let bps = c.read_u16::<LittleEndian>()? as u64;
    let spc = c.read_u8()? as u64;
    c.set_position(0x30);
    let mft_cluster = c.read_u64::<LittleEndian>()?;
    let bytes_per_cluster = bps * spc;
    Ok(NtfsBoot { bytes_per_cluster, mft_offset: mft_cluster * bytes_per_cluster })
}

fn scan_mft(
    reader: &mut dyn DiskReader,
    boot: &NtfsBoot,
    progress_cb: &mut dyn FnMut(ScanProgress),
    stop_flag: Arc<AtomicBool>,
) -> Result<Vec<RecoveredFile>> {
    let mut results = Vec::new();
    let mut record_buf = vec![0u8; MFT_RECORD_SIZE];
    let disk_size = reader.size();
    let mut mft_offset = boot.mft_offset;
    loop {
        if stop_flag.load(Ordering::Relaxed) { break; }
        if mft_offset + MFT_RECORD_SIZE as u64 > disk_size { break; }
        let n = reader.read_at(mft_offset, &mut record_buf)?;
        if n < MFT_RECORD_SIZE { break; }
        if &record_buf[0..4] != MFT_RECORD_SIG { break; }
        let flags = u16::from_le_bytes([record_buf[0x16], record_buf[0x17]]);
        let in_use = (flags & 0x0001) != 0;
        let is_dir = (flags & 0x0002) != 0;
        if !in_use && !is_dir {
            if let Some(recovered) = parse_mft_record(&record_buf, mft_offset, results.len()) {
                results.push(recovered);
                progress_cb(ScanProgress {
                    bytes_scanned: mft_offset, bytes_total: disk_size,
                    files_found: results.len(),
                    phase: format!("Quick scan (NTFS): {} deleted files found", results.len()),
                    complete: false, warning: None,
                });
            }
        }
        mft_offset += MFT_RECORD_SIZE as u64;
    }
    progress_cb(ScanProgress {
        bytes_scanned: disk_size, bytes_total: disk_size, files_found: results.len(),
        phase: "NTFS quick scan complete".to_string(), complete: true, warning: None,
    });
    Ok(results)
}

fn parse_mft_record(buf: &[u8], record_offset: u64, index: usize) -> Option<RecoveredFile> {
    use super::attribute::find_filename_attribute;
    let first_attr = u16::from_le_bytes([buf[0x14], buf[0x15]]) as usize;
    if first_attr >= MFT_RECORD_SIZE { return None; }
    let (name, size) = find_filename_attribute(&buf[first_attr..]);
    let file_type = name.as_deref().map(guess_type).unwrap_or(FileType::Unknown);
    Some(RecoveredFile {
        name, original_path: None, size, file_type, disk_offset: record_offset,
        recovery_method: RecoveryMethod::NtfsMft, confidence: 0.85, index,
    })
}

fn guess_type(name: &str) -> FileType {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => FileType::Jpeg,
        "png" => FileType::Png,
        "gif" => FileType::Gif,
        "bmp" => FileType::Bmp,
        "mp4" => FileType::Mp4,
        "mov" => FileType::Mov,
        "avi" => FileType::Avi,
        "mkv" => FileType::Mkv,
        "pdf" => FileType::Pdf,
        "docx" => FileType::Docx,
        "zip" => FileType::Zip,
        "mp3" => FileType::Mp3,
        _ => FileType::Unknown,
    }
}
