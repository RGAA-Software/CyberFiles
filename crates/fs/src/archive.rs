//! Create zip archives (Windows: Compress-Archive; aligns with Files «Compress»).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Builds `destination_dir / {name}.zip` containing all `sources`.
#[cfg(windows)]
pub fn compress_paths_to_zip(sources: &[PathBuf], destination_dir: &Path) -> anyhow::Result<PathBuf> {
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

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .status()?;

    if !status.success() {
        anyhow::bail!("Compress-Archive failed ({status})");
    }
    Ok(zip_path)
}

#[cfg(not(windows))]
pub fn compress_paths_to_zip(_sources: &[PathBuf], _destination_dir: &Path) -> anyhow::Result<PathBuf> {
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
