//! Recovar CLI — Command-line file recovery tool
//! Run as Administrator for raw disk access!

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use recovarcorelib::{
    RecoverySession,
    types::{FileType, RecoveredFile, RecoveryMethod, ScanMode, ScanProgress, Target},
    disk,
    android::adb,
};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(
    name = "recovar",
    version = env!("CARGO_PKG_VERSION"),
    about = "Recover deleted files from Windows drives and Android devices",
)]
struct Cli {
    #[arg(short, long, global = true)]
    verbose: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scan a drive or Android device for recoverable files
    Scan(ScanArgs),
    /// List available drives or connected Android devices
    List(ListArgs),
    /// Show version and system info
    Info,
}

#[derive(Parser, Debug)]
struct ScanArgs {
    #[arg(short = 't', long, default_value = "laptop")]
    target: TargetArg,
    #[arg(short = 'd', long, default_value = "C:\\")]
    drive: String,
    #[arg(short = 'm', long, default_value = "both")]
    mode: ModeArg,
    #[arg(short = 'o', long, default_value = "./recovered")]
    output: PathBuf,
    #[arg(long)]
    device: Option<String>,
    #[arg(long)]
    image: Option<PathBuf>,
    #[arg(short = 'f', long)]
    filter: Option<String>,
    #[arg(short = 's', long)]
    save: bool,
    #[arg(long, default_value = "0")]
    limit: usize,
}

#[derive(Parser, Debug)]
struct ListArgs {
    #[arg(short = 't', long, default_value = "drives")]
    target: String,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
enum TargetArg { Laptop, Android }

#[derive(ValueEnum, Clone, Debug)]
enum ModeArg { Quick, Deep, Both }

impl From<ModeArg> for ScanMode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::Quick => ScanMode::QuickScan,
            ModeArg::Deep => ScanMode::DeepScan,
            ModeArg::Both => ScanMode::Both,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level))
        )
        .with_target(false).compact().init();
    print_banner();
    match cli.command {
        Commands::Scan(args) => run_scan(args).await,
        Commands::List(args) => run_list(args),
        Commands::Info => run_info(),
    }
}

async fn run_scan(args: ScanArgs) -> Result<()> {
    let mode = ScanMode::from(args.mode);
    let target = match args.target {
        TargetArg::Laptop => Target::Laptop { drive: args.drive.clone() },
        TargetArg::Android => Target::Android {
            serial: args.device.clone(),
            image_path: args.image.as_ref().and_then(|p| p.to_str().map(|s| s.to_string())),
        },
    };

    // FIXED: Extract the drive string here, BEFORE we create and borrow `session`
    let drive_str = match &target {
        Target::Laptop { drive } => drive.clone(),
        Target::Android { .. } => String::new(),
    };

    println!(
        "{}  Starting {} scan on {}",
        "→".cyan().bold(),
        mode.to_string().bold(),
        match &target {
            Target::Laptop { drive } => drive.yellow().to_string(),
            Target::Android { serial, .. } => format!("Android {}", serial.as_deref().unwrap_or("(first device)")).yellow().to_string(),
        }
    );
    println!("{}  Output: {}", "→".cyan(), args.output.display().to_string().yellow());
    println!();
    if args.target == TargetArg::Laptop && !has_disk_privileges() {
        eprintln!("{} {} for full disk access.", "⚠ WARNING:".yellow().bold(), elevation_hint());
        eprintln!();
    }
    
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} [{bar:45.cyan/blue}] {percent:>3}% | {msg}")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ ")
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    
    let mut session = RecoverySession::new(target, mode);
    let start = Instant::now();
    let pb_clone = pb.clone();
    
    let results = session.run(&args.output, &mut move |progress: ScanProgress| {
        pb_clone.set_position(progress.percent() as u64);
        pb_clone.set_message(format!("{} | {} found", progress.phase, progress.files_found));
    })?;
    
    pb.finish_and_clear();
    let elapsed = start.elapsed();
    
    println!(
        "{}  Scan complete in {:.1}s — {} file(s) found",
        "✓".green().bold(), elapsed.as_secs_f32(),
        results.len().to_string().green().bold()
    );
    println!();
    
    if results.is_empty() {
        println!("  {} No recoverable files found.", "●".dimmed());
        println!("  Tip: Try {} mode for deeper scanning.", "--mode deep".yellow());
        return Ok(());
    }
    
    let filter_types = parse_filter(args.filter.as_deref());
    
    // FIXED: Added `.cloned()` to take ownership of the structs. 
    // This drops the mutable borrow on `session` immediately.
    let filtered: Vec<RecoveredFile> = results.iter()
        .filter(|f| {
            if filter_types.is_empty() { f.file_type.is_image() || f.file_type.is_video() }
            else { filter_types.contains(&f.file_type) }
        })
        .take(if args.limit == 0 { usize::MAX } else { args.limit })
        .cloned() 
        .collect();
        
    println!("  {:<6} {:<30} {:>10} {:<18} {:<14} {}",
        "#".bold(), "Name".bold(), "Size".bold(), "Type".bold(), "Method".bold(), "Confidence".bold());
    println!("  {}", "─".repeat(88).dimmed());
    
    for file in &filtered {
        let name = file.name.as_deref().unwrap_or("(unnamed)").chars().take(29).collect::<String>();
        let size = format_size(file.size);
        let method = match &file.recovery_method {
            RecoveryMethod::NtfsMft => "NTFS MFT",
            RecoveryMethod::Fat32Directory => "FAT32 Dir",
            RecoveryMethod::DiskCarving => "Carving",
            RecoveryMethod::AdbPull => "ADB Pull",
        };
        let conf = format!("{:.0}%", file.confidence * 100.0);
        let conf_col = if file.confidence >= 0.9 { conf.green().to_string() }
            else if file.confidence >= 0.7 { conf.yellow().to_string() }
            else { conf.red().to_string() };
            
        println!("  {:<6} {:<30} {:>10} {:<18} {:<14} {}",
            format!("#{}", file.index).dimmed(), name,
            size.cyan().to_string(), file.file_type.display_name(), method, conf_col);
    }
    println!();
    
    if args.save {
        println!("{} Saving {} file(s)...", "→".cyan(), filtered.len());
        let mut saved = 0;
        
        for file in &filtered {
            // FIXED: Using `drive_str` instead of looking back into `session.target`
            match session.save_file(file, &args.output, &drive_str) {
                Ok(path) => { println!("  {} {}", "✓".green(), path.display()); saved += 1; }
                Err(e) => println!("  {} Failed: {}", "✗".red(), e),
            }
        }
        println!("\n{}  {}/{} file(s) saved to {}",
            "✓".green().bold(), saved, filtered.len(), args.output.display());
    } else {
        println!("  {} Use {} to save these files.", "ℹ".blue(), "--save".yellow().bold());
    }
    Ok(())
}

fn run_list(args: ListArgs) -> Result<()> {
    match args.target.as_str() {
        "drives" | "drive" => {
            let drives = disk::list_drives()?;
            println!("\n  {} Available drives:\n", "→".cyan().bold());
            if drives.is_empty() { println!("  No drives found."); }
            for d in &drives {
                println!("  {} {} — {} [{}]", "●".cyan(), d.path.bold(), d.label, d.filesystem.dimmed());
            }
        }
        _ => {
            println!("\n  {} Scanning for Android devices...\n", "→".cyan().bold());
            match adb::list_devices() {
                Ok(devs) if !devs.is_empty() => {
                    for d in &devs {
                        println!("  {} {} ({}) — {}", "●".green(), d.serial.bold(), d.model, d.state.green());
                    }
                }
                Ok(_) => println!("  No Android devices found. Enable USB Debugging on your phone."),
                Err(e) => println!("  {} ADB error: {}", "✗".red(), e),
            }
        }
    }
    Ok(())
}

fn run_info() -> Result<()> {
    println!("  Recovar v{}", env!("CARGO_PKG_VERSION").bold());
    println!("  Platform: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    println!();
    println!("  {}", if has_disk_privileges() {
        format!("Elevated: {}", "Yes".green())
    } else {
        format!("Elevated: {} — {}", "No".red(), elevation_hint())
    });
    println!("  ADB:    {}",
        if adb::check_adb_available() { "Available".green().to_string() } else { "Not found — install Android Platform Tools".red().to_string() });
    let drives = disk::list_drives().unwrap_or_default();
    println!("  Drives: {}", drives.len());
    for d in &drives {
        println!("    {} {} [{}]", "●".dimmed(), d.path, d.filesystem);
    }
    Ok(())
}

fn print_banner() {
    println!();
    println!("{}", "  ██████╗ ███████╗ ██████╗ ██████╗ ██╗   ██╗ █████╗ ██████╗ ".cyan().bold());
    println!("{}", "  ██╔══██╗██╔════╝██╔════╝██╔═══██╗██║   ██║██╔══██╗██╔══██╗".cyan());
    println!("{}", "  ██████╔╝█████╗  ██║     ██║   ██║██║   ██║███████║██████╔╝".cyan());
    println!("{}", "  ██╔══██╗██╔══╝  ██║     ██║   ██║╚██╗ ██╔╝██╔══██║██╔══██╗".blue());
    println!("{}", "  ██║  ██║███████╗╚██████╗╚██████╔╝ ╚████╔╝ ██║  ██║██║  ██║".blue());
    println!("{}", "  ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚═════╝   ╚═══╝  ╚═╝  ╚═╝╚═╝  ╚═╝".blue());
    println!();
    println!("  {} {}", "File Recovery Tool".bold(), format!("v{}", env!("CARGO_PKG_VERSION")).dimmed());
    println!();
}

fn parse_filter(filter: Option<&str>) -> Vec<FileType> {
    let Some(f) = filter else { return vec![]; };
    f.split(',').map(|ext| match ext.trim().to_lowercase().as_str() {
        "jpg" | "jpeg" => FileType::Jpeg, "png" => FileType::Png, "gif" => FileType::Gif,
        "bmp" => FileType::Bmp, "mp4" => FileType::Mp4, "mov" => FileType::Mov,
        "avi" => FileType::Avi, "mkv" => FileType::Mkv, "pdf" => FileType::Pdf,
        "docx" => FileType::Docx, "zip" => FileType::Zip, "mp3" => FileType::Mp3,
        _ => FileType::Unknown,
    }).collect()
}

fn format_size(bytes: u64) -> String {
    if bytes == 0 { return "unknown".to_string(); }
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 { size /= 1024.0; unit += 1; }
    format!("{:.1} {}", size, UNITS[unit])
}

#[cfg(windows)]
fn has_disk_privileges() -> bool {
    use windows_sys::Win32::UI::Shell::IsUserAnAdmin;
    unsafe { IsUserAnAdmin() != 0 }
}

#[cfg(unix)]
fn has_disk_privileges() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(windows)]
fn elevation_hint() -> &'static str { "Run as Administrator" }

#[cfg(unix)]
fn elevation_hint() -> &'static str { "Run with sudo" }