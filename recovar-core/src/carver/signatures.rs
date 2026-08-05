use crate::types::FileType;

#[derive(Debug, Clone)]
pub struct Signature {
    pub header: &'static [u8],
    pub footer: Option<&'static [u8]>,
    pub max_size: usize,
    pub file_type: FileType,
    pub name: &'static str,
}

pub const SIGNATURES: &[Signature] = &[
    Signature { header: &[0xFF, 0xD8, 0xFF], footer: Some(&[0xFF, 0xD9]), max_size: 50_000_000, file_type: FileType::Jpeg, name: "JPEG" },
    Signature { header: &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], footer: Some(&[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]), max_size: 100_000_000, file_type: FileType::Png, name: "PNG" },
    Signature { header: &[0x47, 0x49, 0x46, 0x38, 0x37, 0x61], footer: Some(&[0x00, 0x3B]), max_size: 20_000_000, file_type: FileType::Gif, name: "GIF87a" },
    Signature { header: &[0x47, 0x49, 0x46, 0x38, 0x39, 0x61], footer: Some(&[0x00, 0x3B]), max_size: 20_000_000, file_type: FileType::Gif, name: "GIF89a" },
    Signature { header: &[0x42, 0x4D], footer: None, max_size: 50_000_000, file_type: FileType::Bmp, name: "BMP" },
    Signature { header: &[0x00, 0x00, 0x00, 0x18, 0x66, 0x74, 0x79, 0x70], footer: None, max_size: 4_000_000_000, file_type: FileType::Mp4, name: "MP4" },
    Signature { header: &[0x00, 0x00, 0x00, 0x14, 0x66, 0x74, 0x79, 0x70], footer: None, max_size: 4_000_000_000, file_type: FileType::Mp4, name: "MP4 small" },
    Signature { header: &[0x52, 0x49, 0x46, 0x46], footer: None, max_size: 2_000_000_000, file_type: FileType::Avi, name: "AVI" },
    Signature { header: &[0x1A, 0x45, 0xDF, 0xA3], footer: None, max_size: 4_000_000_000, file_type: FileType::Mkv, name: "MKV" },
    Signature { header: &[0x25, 0x50, 0x44, 0x46, 0x2D], footer: Some(&[0x25, 0x25, 0x45, 0x4F, 0x46]), max_size: 500_000_000, file_type: FileType::Pdf, name: "PDF" },
    Signature { header: &[0x50, 0x4B, 0x03, 0x04], footer: Some(&[0x50, 0x4B, 0x05, 0x06]), max_size: 500_000_000, file_type: FileType::Docx, name: "DOCX/ZIP" },
    Signature { header: &[0x49, 0x44, 0x33], footer: None, max_size: 100_000_000, file_type: FileType::Mp3, name: "MP3" },
];

pub fn match_signature(data: &[u8]) -> Option<&'static Signature> {
    SIGNATURES.iter().find(|sig| data.starts_with(sig.header))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_signature_jpeg() {
        let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
        let matched = match_signature(&jpeg_data);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "JPEG");
    }

    #[test]
    fn test_match_signature_png() {
        let png_data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
        let matched = match_signature(&png_data);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "PNG");
    }

    #[test]
    fn test_match_signature_unknown() {
        let random_data = [0x00, 0x11, 0x22, 0x33, 0x44];
        let matched = match_signature(&random_data);
        assert!(matched.is_none());
    }
}

