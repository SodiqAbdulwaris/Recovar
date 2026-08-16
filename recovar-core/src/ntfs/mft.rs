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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Builds a minimal NTFS boot sector plus one deleted MFT record
    /// containing a resident $FILE_NAME attribute for "test.png".
    fn build_ntfs_image() -> Vec<u8> {
        let mut img = vec![0u8; 2048];

        // Boot sector: NTFS signature, 512 bytes/sector, 1 sector/cluster,
        // MFT starts at cluster 2 (byte offset 1024).
        img[3..11].copy_from_slice(NTFS_SIGNATURE);
        img[0x0B..0x0D].copy_from_slice(&512u16.to_le_bytes());
        img[0x0D] = 1;
        img[0x30..0x38].copy_from_slice(&2u64.to_le_bytes());

        // MFT record at offset 1024 (cluster 2): in-use bit and dir bit both
        // clear, i.e. a deleted, non-directory file - what scan_mft targets.
        let rec = 1024usize;
        img[rec..rec + 4].copy_from_slice(MFT_RECORD_SIG);
        img[rec + 0x14..rec + 0x16].copy_from_slice(&56u16.to_le_bytes()); // first attr offset
        img[rec + 0x16..rec + 0x18].copy_from_slice(&0u16.to_le_bytes()); // flags: deleted file

        // $FILE_NAME attribute (type 0x30), resident, starting at rec+56.
        let attr = rec + 56;
        let attr_len: u32 = 112;
        img[attr..attr + 4].copy_from_slice(&0x30u32.to_le_bytes()); // attr type
        img[attr + 4..attr + 8].copy_from_slice(&attr_len.to_le_bytes()); // attr len
        img[attr + 8] = 0; // resident
        let content_off: u16 = 24;
        img[attr + 20..attr + 22].copy_from_slice(&content_off.to_le_bytes());

        let content = attr + content_off as usize;
        let real_size: u64 = 12345;
        img[content + 0x30..content + 0x38].copy_from_slice(&real_size.to_le_bytes());
        let name = "test.png";
        img[content + 0x40] = name.chars().count() as u8;
        let name_start = content + 0x42;
        for (i, c) in name.encode_utf16().enumerate() {
            img[name_start + i * 2..name_start + i * 2 + 2].copy_from_slice(&c.to_le_bytes());
        }

        // Attribute list terminator right after this attribute.
        img[attr + attr_len as usize..attr + attr_len as usize + 4].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());

        img
    }

    #[test]
    fn recovers_deleted_ntfs_file_name_and_size() {
        let img = build_ntfs_image();
        let mut disk = MemDisk(img);
        let stop = Arc::new(AtomicBool::new(false));

        let results = scan_ntfs(&mut disk, &mut |_| {}, stop).expect("scan should succeed");

        assert_eq!(results.len(), 1, "expected exactly one deleted MFT record to be found");
        let file = &results[0];
        assert_eq!(file.name.as_deref(), Some("test.png"));
        assert_eq!(file.size, 12345);
        assert_eq!(file.file_type, FileType::Png);
        assert_eq!(file.disk_offset, 1024, "disk_offset must point at the MFT record itself");
    }
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
