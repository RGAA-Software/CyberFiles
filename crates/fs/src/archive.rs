//! Create zip archives (Windows: Compress-Archive; aligns with Files «Compress»).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Returned when the user cancels during compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressCancelled;

impl std::fmt::Display for CompressCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "compress cancelled")
    }
}

impl std::error::Error for CompressCancelled {}

/// Builds `destination_dir / {name}.zip` containing all `sources`.
pub fn compress_paths_to_zip(sources: &[PathBuf], destination_dir: &Path) -> anyhow::Result<PathBuf> {
    compress_paths_to_zip_cancellable(sources, destination_dir, &AtomicBool::new(false))
}

/// Like [`compress_paths_to_zip`], but polls `cancel` and kills the compressor process.
pub fn compress_paths_to_zip_cancellable(
    sources: &[PathBuf],
    destination_dir: &Path,
    cancel: &AtomicBool,
) -> anyhow::Result<PathBuf> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CompressCancelled.into());
    }
    compress_paths_to_zip_impl(sources, destination_dir, cancel)
}

#[cfg(windows)]
use std::process::{Command, Stdio};

#[cfg(windows)]
fn compress_paths_to_zip_impl(
    sources: &[PathBuf],
    destination_dir: &Path,
    cancel: &AtomicBool,
) -> anyhow::Result<PathBuf> {
    if sources.is_empty() {
        anyhow::bail!("no paths to compress");
    }
    if !destination_dir.is_dir() {
        anyhow::bail!("destination is not a directory");
    }

    let zip_name = zip_file_name(sources);
    let zip_path = destination_dir.join(&zip_name);
    if zip_path.exists() {
        std::fs::remove_file(&zip_path)?;
    }

    let mut script = String::from("$ErrorActionPreference='Stop'; Compress-Archive -Force");
    script.push_str(" -DestinationPath ");
    script.push_str(&powershell_literal(&zip_path));
    script.push_str(" -LiteralPath ");
    let literals: Vec<String> = sources
        .iter()
        .map(|p| powershell_literal(p))
        .collect();
    script.push_str(&literals.join(","));

    let mut child = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            if zip_path.exists() {
                let _ = std::fs::remove_file(&zip_path);
            }
            return Err(CompressCancelled.into());
        }
        match child.try_wait()? {
            Some(status) => {
                if !status.success() {
                    anyhow::bail!("Compress-Archive failed ({status})");
                }
                return Ok(zip_path);
            }
            None => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

#[cfg(not(windows))]
fn compress_paths_to_zip_impl(
    _sources: &[PathBuf],
    _destination_dir: &Path,
    _cancel: &AtomicBool,
) -> anyhow::Result<PathBuf> {
    anyhow::bail!("compress is only supported on Windows")
}

#[cfg(windows)]
fn zip_file_name(sources: &[PathBuf]) -> String {
    if sources.len() == 1 {
        let stem = sources[0]
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Archive".into());
        return format!("{stem}.zip");
    }
    "Archive.zip".into()
}

#[cfg(windows)]
fn powershell_literal(path: &Path) -> String {
    let raw = path.to_string_lossy();
    format!("'{}'", raw.replace('\'', "''"))
}
