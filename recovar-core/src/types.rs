use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanMode {
    QuickScan,
    DeepScan,
    Both,
}

impl std::fmt::Display for ScanMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanMode::QuickScan => write!(f, "quick"),
            ScanMode::DeepScan => write!(f, "deep"),
            ScanMode::Both => write!(f, "both"),
        }
    }
}

impl std::str::FromStr for ScanMode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "quick" => Ok(ScanMode::QuickScan),
            "deep" => Ok(ScanMode::DeepScan),
            "both" => Ok(ScanMode::Both),
            _ => Err(anyhow::anyhow!("Unknown scan mode: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Target {
    Laptop { drive: String },
    Android { serial: Option<String>, image_path: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Jpeg, Png, Gif, Bmp, Mp4, Mov, Avi, Mkv, Pdf, Docx, Zip, Mp3, Unknown,
}

impl FileType {
    pub fn extension(&self) -> &'static str {
        match self {
            FileType::Jpeg => "jpg",
            FileType::Png => "png",
            FileType::Gif => "gif",
            FileType::Bmp => "bmp",
            FileType::Mp4 => "mp4",
            FileType::Mov => "mov",
            FileType::Avi => "avi",
            FileType::Mkv => "mkv",
            FileType::Pdf => "pdf",
            FileType::Docx => "docx",
            FileType::Zip => "zip",
            FileType::Mp3 => "mp3",
            FileType::Unknown => "bin",
        }
    }

    pub fn is_image(&self) -> bool {
        matches!(self, FileType::Jpeg | FileType::Png | FileType::Gif | FileType::Bmp)
    }

    pub fn is_video(&self) -> bool {
        matches!(self, FileType::Mp4 | FileType::Mov | FileType::Avi | FileType::Mkv)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            FileType::Jpeg => "JPEG Image",
            FileType::Png => "PNG Image",
            FileType::Gif => "GIF Image",
            FileType::Bmp => "BMP Image",
            FileType::Mp4 => "MP4 Video",
            FileType::Mov => "MOV Video",
            FileType::Avi => "AVI Video",
            FileType::Mkv => "MKV Video",
            FileType::Pdf => "PDF Document",
            FileType::Docx => "Word Document",
            FileType::Zip => "ZIP Archive",
            FileType::Mp3 => "MP3 Audio",
            FileType::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveredFile {
    pub name: Option<String>,
    pub original_path: Option<String>,
    pub size: u64,
    pub file_type: FileType,
    pub disk_offset: u64,
    pub recovery_method: RecoveryMethod,
    pub confidence: f32,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryMethod {
    NtfsMft,
    Fat32Directory,
    DiskCarving,
    AdbPull,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub bytes_scanned: u64,
    pub bytes_total: u64,
    pub files_found: usize,
    pub phase: String,
    pub complete: bool,
    pub warning: Option<String>,
}

impl ScanProgress {
    pub fn percent(&self) -> f32 {
        if self.bytes_total == 0 { return 0.0; }
        (self.bytes_scanned as f32 / self.bytes_total as f32 * 100.0).min(100.0)
    }
}
