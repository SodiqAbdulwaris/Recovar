pub fn find_filename_attribute(attrs: &[u8]) -> (Option<String>, u64) {
    let mut pos = 0usize;
    let mut filename: Option<String> = None;
    let mut size: u64 = 0;
    while pos + 8 <= attrs.len() {
        let attr_type = u32::from_le_bytes([attrs[pos], attrs[pos+1], attrs[pos+2], attrs[pos+3]]);
        if attr_type == 0xFFFF_FFFF { break; }
        let attr_len = u32::from_le_bytes([attrs[pos+4], attrs[pos+5], attrs[pos+6], attrs[pos+7]]) as usize;
        if attr_len == 0 || pos + attr_len > attrs.len() { break; }
        let non_resident = if pos + 8 < attrs.len() { attrs[pos + 8] } else { 1 };
        if attr_type == 0x30 && non_resident == 0 && pos + 22 <= attrs.len() {
            let content_off = u16::from_le_bytes([attrs[pos+20], attrs[pos+21]]) as usize;
            let cs = pos + content_off;
            if cs + 0x42 <= attrs.len() {
                size = u64::from_le_bytes(attrs[cs+0x30..cs+0x38].try_into().unwrap_or([0;8]));
                let name_len = attrs[cs + 0x40] as usize;
                let name_start = cs + 0x42;
                let name_end = name_start + name_len * 2;
                if name_end <= attrs.len() {
                    let utf16: Vec<u16> = attrs[name_start..name_end].chunks_exact(2)
                        .map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
                    filename = Some(String::from_utf16_lossy(&utf16).to_string());
                }
            }
        }
        pos += attr_len;
    }
    (filename, size)
}
