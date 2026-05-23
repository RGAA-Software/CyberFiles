use std::path::Path;

const MAX_TEXT_PREVIEW_BYTES: usize = 64 * 1024;

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tif", "tiff",
];

const TEXT_EXTENSIONS: &[&str] = &[
    "txt",
    "md",
    "json",
    "xml",
    "yaml",
    "yml",
    "toml",
    "rs",
    "log",
    "csv",
    "ini",
    "cfg",
    "html",
    "htm",
    "css",
    "js",
    "ts",
    "tsx",
    "jsx",
    "py",
    "c",
    "cpp",
    "h",
    "hpp",
    "cs",
    "java",
    "go",
    "sql",
    "sh",
    "bat",
    "ps1",
    "gitignore",
];

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn is_text_preview_path(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| TEXT_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn read_text_preview(path: &Path) -> anyhow::Result<String> {
    let data = std::fs::read(path)?;
    let truncated = data.len() > MAX_TEXT_PREVIEW_BYTES;
    let slice = &data[..data.len().min(MAX_TEXT_PREVIEW_BYTES)];
    let mut text = String::from_utf8_lossy(slice).into_owned();
    if truncated {
        text.push_str("\n…");
    }
    Ok(text)
}
