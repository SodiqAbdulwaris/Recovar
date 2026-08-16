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
                if let Some(r) = parse_entry(entry, bpb, results.len()) {
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

fn parse_entry(entry: &[u8], bpb: &Fat32Bpb, index: usize) -> Option<RecoveredFile> {
    let raw_name = std::str::from_utf8(&entry[1..8]).unwrap_or("").trim().to_string();
    let raw_ext = std::str::from_utf8(&entry[8..11]).unwrap_or("").trim().to_lowercase();
    if raw_name.is_empty() { return None; }
    let filename = if raw_ext.is_empty() { format!("?{raw_name}") } else { format!("?{raw_name}.{raw_ext}") };
    let size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]) as u64;
    // The deleted-entry marker only clobbers the first name byte; the cluster
    // fields (start of file data) survive, so the content can still be pulled.
    let cluster_hi = u16::from_le_bytes([entry[20], entry[21]]) as u32;
    let cluster_lo = u16::from_le_bytes([entry[26], entry[27]]) as u32;
    let first_cluster = (cluster_hi << 16) | cluster_lo;
    let disk_offset = if first_cluster >= 2 { cluster_offset(bpb, first_cluster) } else { 0 };
    let file_type = match raw_ext.as_str() {
        "jpg" | "jpeg" => FileType::Jpeg, "png" => FileType::Png, "gif" => FileType::Gif,
        "bmp" => FileType::Bmp, "mp4" => FileType::Mp4, "mov" => FileType::Mov,
        "avi" => FileType::Avi, "mkv" => FileType::Mkv, "pdf" => FileType::Pdf,
        "docx" => FileType::Docx, "zip" => FileType::Zip, "mp3" => FileType::Mp3,
        _ => FileType::Unknown,
    };
    Some(RecoveredFile {
        name: Some(filename), original_path: None, size, file_type, disk_offset,
        recovery_method: RecoveryMethod::Fat32Directory, confidence: 0.80, index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// An in-memory disk backed by a byte buffer, so scanning logic can be
    /// exercised without touching a real Windows device or needing Administrator.
    struct MemDisk(Vec<u8>);

    impl DiskReader for MemDisk {
        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> anyhow::Result<usize> {
            let offset = offset as usize;
            if offset >= self.0.len() { return Ok(0); }
            let n = buf.len().min(self.0.len() - offset);
            buf[..n].copy_from_slice(&self.0[offset..offset + n]);
            Ok(n)
        }
        fn size(&self) -> u64 { self.0.len() as u64 }
        fn sector_size(&self) -> u32 { 512 }
    }

    /// Builds a minimal FAT32 image (boot sector + one deleted file in the
    /// root directory) and returns it along with the original file bytes.
    fn build_fat32_image() -> (Vec<u8>, Vec<u8>) {
        let mut img = vec![0u8; 2048];

        // Boot sector / BPB: 512 bytes/sector, 1 sector/cluster, 1 reserved
        // sector, 1 FAT of 1 sector, root directory starts at cluster 2.
        img[0x0B..0x0D].copy_from_slice(&512u16.to_le_bytes());
        img[0x0D] = 1;
        img[0x0E..0x10].copy_from_slice(&1u16.to_le_bytes());
        img[0x10] = 1;
        img[0x24..0x28].copy_from_slice(&1u32.to_le_bytes());
        img[0x2C..0x30].copy_from_slice(&2u32.to_le_bytes());
        img[82..90].copy_from_slice(b"FAT32   ");
        img[510] = 0x55;
        img[511] = 0xAA;

        // Root directory cluster (offset 1024): one deleted 8.3 entry
        // "TESTFIL.JPG", 300 bytes, starting at cluster 3.
        let e = 1024;
        img[e] = DELETED_MARKER;
        img[e + 1..e + 8].copy_from_slice(b"TESTFIL");
        img[e + 8..e + 11].copy_from_slice(b"JPG");
        img[e + 11] = 0x20; // ATTR_ARCHIVE
        img[e + 20..e + 22].copy_from_slice(&0u16.to_le_bytes()); // cluster high
        img[e + 26..e + 28].copy_from_slice(&3u16.to_le_bytes()); // cluster low
        img[e + 28..e + 32].copy_from_slice(&300u32.to_le_bytes()); // size

        // File data cluster (offset 1536 = data_start + (3-2)*512).
        let payload: Vec<u8> = (0..300u32).map(|i| b'A' + (i % 26) as u8).collect();
        img[1536..1536 + 300].copy_from_slice(&payload);

        (img, payload)
    }

    #[test]
    fn recovers_deleted_fat32_file_with_correct_bytes() {
        let (img, expected_payload) = build_fat32_image();
        let mut disk = MemDisk(img);
        let stop = Arc::new(AtomicBool::new(false));

        let results = scan_fat32(&mut disk, &mut |_| {}, stop).expect("scan should succeed");

        assert_eq!(results.len(), 1, "expected exactly one deleted file to be found");
        let file = &results[0];
        assert_eq!(file.name.as_deref(), Some("?TESTFIL.jpg"));
        assert_eq!(file.size, 300);
        assert_eq!(
            file.disk_offset, 1536,
            "disk_offset must point at the file's actual data cluster, not 0"
        );

        // Mirrors what RecoverySession::save_file does: read `size` bytes at `disk_offset`.
        let mut recovered = vec![0u8; file.size as usize];
        let n = disk.read_at(file.disk_offset, &mut recovered).unwrap();
        assert_eq!(n, 300);
        assert_eq!(
            recovered, expected_payload,
            "recovered bytes must match the original file content"
        );
    }
}
