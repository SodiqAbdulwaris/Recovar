use crate::disk::DiskReader;
use crate::types::{FileType, RecoveredFile, RecoveryMethod, ScanProgress};
use anyhow::{bail, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const DIR_ENTRY_SIZE: usize = 32;
const DELETED_MARKER: u8 = 0xE5;

struct Fat32Bpb {
    bytes_per_cluster: u32,
    data_start: u64,
    root_cluster: u32,
}

pub fn scan_fat32(
    reader: &mut dyn DiskReader,
    progress_cb: &mut dyn FnMut(ScanProgress),
    stop_flag: Arc<AtomicBool>,
) -> Result<Vec<RecoveredFile>> {
    let mut boot_buf = vec![0u8; 512];
    reader.read_at(0, &mut boot_buf)?;
    let sig = u16::from_le_bytes([boot_buf[510], boot_buf[511]]);
    if sig != 0xAA55 { bail!("Not a valid FAT boot sector"); }
    let fs_type = std::str::from_utf8(&boot_buf[82..90]).unwrap_or("");
    if !fs_type.starts_with("FAT") { bail!("Not a FAT32 filesystem: {}", fs_type); }
    let bpb = parse_bpb(&boot_buf)?;
    progress_cb(ScanProgress {
        bytes_scanned: 0, bytes_total: reader.size(), files_found: 0,
        phase: "Quick scan: reading FAT32 directory entries".to_string(), complete: false, warning: None,
    });
    let mut results = Vec::new();
    scan_dir(reader, &bpb, bpb.root_cluster, &mut results, progress_cb, stop_flag, 0)?;
    progress_cb(ScanProgress {
        bytes_scanned: reader.size(), bytes_total: reader.size(), files_found: results.len(),
        phase: "FAT32 quick scan complete".to_string(), complete: true, warning: None,
    });
    Ok(results)
}

fn parse_bpb(buf: &[u8]) -> Result<Fat32Bpb> {
    let mut c = Cursor::new(buf);
    c.set_position(0x0B);
    let bps = c.read_u16::<LittleEndian>()? as u32;
    let spc = c.read_u8()? as u32;
    let reserved = c.read_u16::<LittleEndian>()? as u32;
    let fat_count = c.read_u8()? as u32;
    c.set_position(0x24);
    let spf = c.read_u32::<LittleEndian>()?;
    c.set_position(0x2C);
    let root_cluster = c.read_u32::<LittleEndian>()?;
    let fat_size = fat_count * spf * bps;
    let data_start = (reserved as u64 * bps as u64) + fat_size as u64;
    Ok(Fat32Bpb { bytes_per_cluster: bps * spc, data_start, root_cluster })
}

fn cluster_offset(bpb: &Fat32Bpb, cluster: u32) -> u64 {
    bpb.data_start + (cluster as u64 - 2) * bpb.bytes_per_cluster as u64
}

fn scan_dir(
    reader: &mut dyn DiskReader,
    bpb: &Fat32Bpb,
    cluster: u32,
    results: &mut Vec<RecoveredFile>,
    progress_cb: &mut dyn FnMut(ScanProgress),
    stop_flag: Arc<AtomicBool>,
    depth: usize,
) -> Result<()> {
    if depth > 16 || stop_flag.load(Ordering::Relaxed) { return Ok(()); }
    let off = cluster_offset(bpb, cluster);
    let mut buf = vec![0u8; bpb.bytes_per_cluster as usize];
    let n = reader.read_at(off, &mut buf)?;
    let mut pos = 0usize;
    while pos + DIR_ENTRY_SIZE <= n {
        let entry = &buf[pos..pos + DIR_ENTRY_SIZE];
        if entry[0] == 0x00 { break; }
        if entry[0] == DELETED_MARKER {
            let attrs = entry[11];
            if attrs != 0x0F && (attrs & 0x10) == 0 && (attrs & 0x08) == 0 {
                if let Some(r) = parse_entry(entry, results.len()) {
                    results.push(r);
                    progress_cb(ScanProgress {
                        bytes_scanned: off, bytes_total: reader.size(), files_found: results.len(),
                        phase: format!("Quick scan (FAT32): {} found", results.len()),
                        complete: false, warning: None,
                    });
                }
            }
        }
        pos += DIR_ENTRY_SIZE;
    }
    Ok(())
}

fn parse_entry(entry: &[u8], index: usize) -> Option<RecoveredFile> {
    let raw_name = std::str::from_utf8(&entry[1..8]).unwrap_or("").trim().to_string();
    let raw_ext = std::str::from_utf8(&entry[8..11]).unwrap_or("").trim().to_lowercase();
    if raw_name.is_empty() { return None; }
    let filename = if raw_ext.is_empty() { format!("?{raw_name}") } else { format!("?{raw_name}.{raw_ext}") };
    let size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]) as u64;
    let file_type = match raw_ext.as_str() {
        "jpg" | "jpeg" => FileType::Jpeg, "png" => FileType::Png, "gif" => FileType::Gif,
        "bmp" => FileType::Bmp, "mp4" => FileType::Mp4, "mov" => FileType::Mov,
        "avi" => FileType::Avi, "mkv" => FileType::Mkv, "pdf" => FileType::Pdf,
        "docx" => FileType::Docx, "zip" => FileType::Zip, "mp3" => FileType::Mp3,
        _ => FileType::Unknown,
    };
    Some(RecoveredFile {
        name: Some(filename), original_path: None, size, file_type, disk_offset: 0,
        recovery_method: RecoveryMethod::Fat32Directory, confidence: 0.80, index,
    })
}
