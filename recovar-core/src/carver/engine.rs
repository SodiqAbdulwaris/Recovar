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
