use super::signatures::{match_signature, Signature};
use crate::disk::DiskReader;
use crate::types::{RecoveredFile, RecoveryMethod, ScanProgress};
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const CHUNK_SIZE: usize = 1_024 * 1_024;
const OVERLAP: usize = 16;

pub fn carve(
    reader: &mut dyn DiskReader,
    start_offset: u64,
    progress_cb: &mut dyn FnMut(ScanProgress),
    stop_flag: Arc<AtomicBool>,
) -> Result<Vec<RecoveredFile>> {
    let disk_size = reader.size();
    let mut results: Vec<RecoveredFile> = Vec::new();
    let mut offset = start_offset;
    let mut chunk_buf = vec![0u8; CHUNK_SIZE];
    let mut leftover: Vec<u8> = vec![0u8; 0];
    let mut file_index = 0usize;

    progress_cb(ScanProgress {
        bytes_scanned: 0, bytes_total: disk_size, files_found: 0,
        phase: "Deep scan: carving signatures".to_string(), complete: false, warning: None,
    });

    while offset < disk_size {
        if stop_flag.load(Ordering::Relaxed) { break; }
        let read_len = ((disk_size - offset) as usize).min(CHUNK_SIZE);
        let n = match reader.read_at(offset, &mut chunk_buf[..read_len]) {
            Ok(n) if n > 0 => n,
            _ => break,
        };
        let window: Vec<u8> = leftover.iter().copied().chain(chunk_buf[..n].iter().copied()).collect();
        let window_base = if offset >= OVERLAP as u64 { offset - leftover.len() as u64 } else { 0 };
        let mut pos = 0usize;
        while pos + 1 < window.len() {
            if let Some(sig) = match_signature(&window[pos..]) {
                let abs_offset = window_base + pos as u64;
                if let Some(recovered) = extract_file(reader, abs_offset, sig, file_index) {
                    file_index += 1;
                    results.push(recovered);
                    progress_cb(ScanProgress {
                        bytes_scanned: offset, bytes_total: disk_size,
                        files_found: results.len(),
                        phase: format!("Deep scan: {} found", results.len()),
                        complete: false, warning: None,
                    });
                }
                pos += sig.header.len();
            } else { pos += 1; }
        }
        let keep = window.len().min(OVERLAP);
        leftover = window[window.len() - keep..].to_vec();
        offset += n as u64;
        progress_cb(ScanProgress {
            bytes_scanned: offset, bytes_total: disk_size, files_found: results.len(),
            phase: format!("Deep scan: {:.1}%", offset as f64 / disk_size as f64 * 100.0),
            complete: false, warning: None,
        });
    }

    progress_cb(ScanProgress {
        bytes_scanned: disk_size, bytes_total: disk_size, files_found: results.len(),
        phase: "Deep scan complete".to_string(), complete: true, warning: None,
    });
    Ok(results)
}

fn extract_file(reader: &mut dyn DiskReader, offset: u64, sig: &Signature, index: usize) -> Option<RecoveredFile> {
    let mut probe = vec![0u8; (sig.header.len() + 4).min(16)];
    let n = reader.read_at(offset, &mut probe).ok()?;
    if n < sig.header.len() || !probe[..n].starts_with(sig.header) { return None; }
    let (size, confidence) = if let Some(footer) = sig.footer {
        find_footer(reader, offset, footer, sig.max_size)
    } else {
        (0u64, 0.6)
    };
    Some(RecoveredFile {
        name: None, original_path: None, size,
        file_type: sig.file_type.clone(),
        disk_offset: offset,
        recovery_method: RecoveryMethod::DiskCarving,
        confidence, index,
    })
}

fn find_footer(reader: &mut dyn DiskReader, start: u64, footer: &[u8], max_size: usize) -> (u64, f32) {
    const SCAN_BUF: usize = 64 * 1024;
    let mut buf = vec![0u8; SCAN_BUF];
    let mut pos = 0usize;
    let mut prev_tail: Vec<u8> = Vec::new();
    while pos < max_size {
        let n = match reader.read_at(start + pos as u64, &mut buf[..SCAN_BUF]) {
            Ok(n) if n > 0 => n,
            _ => break,
        };
        let window: Vec<u8> = prev_tail.iter().copied().chain(buf[..n].iter().copied()).collect();
        if let Some(idx) = window.windows(footer.len()).position(|w| w == footer) {
            let overlap = prev_tail.len();
            let size = pos as u64 - overlap as u64 + idx as u64 + footer.len() as u64;
            return (size, 0.95);
        }
        let keep = footer.len().saturating_sub(1);
        prev_tail = if n > keep { buf[n - keep..n].to_vec() } else { buf[..n].to_vec() };
        pos += n.saturating_sub(keep);
    }
    (0, 0.4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::DiskReader;

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

    #[test]
    fn carves_deleted_jpeg_out_of_raw_bytes() {
        // No filesystem at all here: some noise, then a JPEG (header ... footer)
        // embedded at a known offset, as if its directory entry were long gone
        // and only the raw signature survives, which is what deep scan targets.
        let mut disk_bytes = vec![0xCCu8; 4096];
        let jpeg_offset = 1000usize;
        let jpeg_body: Vec<u8> = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46]
            .iter().copied()
            .chain((0..200).map(|i| (i % 251) as u8)) // filler "image data"
            .chain([0xFF, 0xD9]) // JPEG footer
            .collect();
        disk_bytes[jpeg_offset..jpeg_offset + jpeg_body.len()].copy_from_slice(&jpeg_body);

        let mut disk = MemDisk(disk_bytes);
        let stop = Arc::new(AtomicBool::new(false));
        let results = carve(&mut disk, 0, &mut |_| {}, stop).expect("carve should succeed");

        assert_eq!(results.len(), 1, "expected exactly one carved file");
        let file = &results[0];
        assert_eq!(file.file_type, crate::types::FileType::Jpeg);
        assert_eq!(file.disk_offset, jpeg_offset as u64);
        assert_eq!(
            file.size as usize,
            jpeg_body.len(),
            "carved size must span header through footer, not just the header"
        );

        let mut recovered = vec![0u8; file.size as usize];
        disk.read_at(file.disk_offset, &mut recovered).unwrap();
        assert_eq!(recovered, jpeg_body, "carved bytes must match the original file");
    }
}
