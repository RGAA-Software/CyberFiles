//! Create zip archives (Rust `zip` crate; per-item progress for the status bar).

use std::fs::File;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

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
    compress_paths_to_zip_cancellable(sources, destination_dir, &AtomicBool::new(false), |_, _| {})
}

/// Like [`compress_paths_to_zip`], but checks `cancel` and reports `on_progress(completed, total)`.
pub fn compress_paths_to_zip_cancellable(
    sources: &[PathBuf],
    destination_dir: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u32, u32),
) -> anyhow::Result<PathBuf> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CompressCancelled.into());
    }
    compress_paths_to_zip_impl(sources, destination_dir, cancel, &mut on_progress)
}

fn compress_paths_to_zip_impl(
    sources: &[PathBuf],
    destination_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<PathBuf> {
    if sources.is_empty() {
        anyhow::bail!("no paths to compress");
    }
    if !destination_dir.is_dir() {
        anyhow::bail!("destination is not a directory");
    }

    for source in sources {
        if !source.exists() {
            anyhow::bail!("path not found: {}", source.display());
        }
    }

    let total = sources.len() as u32;
    on_progress(0, total);

    let zip_path = destination_dir.join(zip_file_name(sources));
    if zip_path.exists() {
        std::fs::remove_file(&zip_path)?;
    }

    let file = File::create(&zip_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for (index, source) in sources.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            let _ = std::fs::remove_file(&zip_path);
            return Err(CompressCancelled.into());
        }
        let entry_name = source
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid source path {}", source.display()))?
            .to_string_lossy()
            .into_owned();
        write_zip_tree(&mut zip, source, &entry_name, &options, cancel)?;
        on_progress((index + 1) as u32, total);
    }

    zip.finish()?;
    Ok(zip_path)
}

fn write_zip_tree<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    path: &Path,
    name_in_archive: &str,
    options: &SimpleFileOptions,
    cancel: &AtomicBool,
) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CompressCancelled.into());
    }

    if path.is_file() {
        zip.start_file(name_in_archive, *options)?;
        let mut file = File::open(path)?;
        std::io::copy(&mut file, zip)?;
        return Ok(());
    }

    if path.is_dir() {
        let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        if entries.is_empty() {
            zip.add_directory(format!("{name_in_archive}/"), *options)?;
            return Ok(());
        }
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return Err(CompressCancelled.into());
            }
            let child_path = entry.path();
            let child_name = entry.file_name().to_string_lossy().into_owned();
            let relative = format!("{name_in_archive}/{child_name}");
            write_zip_tree(zip, &child_path, &relative, options, cancel)?;
        }
        return Ok(());
    }

    anyhow::bail!("unsupported path type: {}", path.display())
}

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
