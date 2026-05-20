use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellIconHint {
    Folder,
    File,
    Symlink,
    Executable,
    Image,
    Archive,
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tif"];
const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "rar", "7z", "tar", "gz", "bz2", "xz"];
const EXECUTABLE_EXTENSIONS: &[&str] = &["exe", "msi", "bat", "cmd", "com", "ps1"];

/// Maps a path to a UI icon category (full Shell icon bitmaps are future work).
pub fn icon_hint_for_path(path: &Path) -> ShellIconHint {
    if path.is_dir() {
        return ShellIconHint::Folder;
    }

    let metadata = std::fs::symlink_metadata(path).ok();
    if metadata.as_ref().is_some_and(|m| m.file_type().is_symlink()) {
        return ShellIconHint::Symlink;
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        Some(e) if IMAGE_EXTENSIONS.contains(&e) => ShellIconHint::Image,
        Some(e) if ARCHIVE_EXTENSIONS.contains(&e) => ShellIconHint::Archive,
        Some(e) if EXECUTABLE_EXTENSIONS.contains(&e) => ShellIconHint::Executable,
        _ => ShellIconHint::File,
    }
}
