use std::collections::HashSet;
use std::path::{Path, PathBuf};

use cyberfiles_core::{load_config, pinned_folder_paths, FileTagConfig};

#[cfg(windows)]
use cyberfiles_platform_windows::list_shell_quick_access_folders;

/// One quick-access folder on the Home page (Shell QA + user pinned).
#[derive(Debug, Clone)]
pub struct QuickAccessEntry {
    pub label: String,
    pub path: PathBuf,
    pub is_pinned: bool,
}

pub fn list_quick_access_entries() -> Vec<QuickAccessEntry> {
    let pinned_set: HashSet<String> = pinned_folder_paths()
        .into_iter()
        .map(|p| path_key(&p))
        .collect();
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    #[cfg(windows)]
    if let Ok(shell) = list_shell_quick_access_folders() {
        for item in shell {
            if item.path.exists() && seen.insert(path_key(&item.path)) {
                entries.push(QuickAccessEntry {
                    label: item.display_name,
                    path: item.path.clone(),
                    is_pinned: pinned_set.contains(&path_key(&item.path)),
                });
            }
        }
    }

    for path in pinned_folder_paths() {
        if !path.exists() || !seen.insert(path_key(&path)) {
            continue;
        }
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        entries.push(QuickAccessEntry {
            label,
            path,
            is_pinned: true,
        });
    }

    entries
}

#[derive(Debug, Clone)]
pub struct FileTagPreview {
    pub tag: FileTagConfig,
    pub preview_items: Vec<(String, PathBuf)>,
}

const TAG_PREVIEW_LIMIT: usize = 8;

pub fn file_tag_previews(tags: &[FileTagConfig]) -> Vec<FileTagPreview> {
    tags.iter()
        .map(|tag| {
            let preview_items: Vec<(String, PathBuf)> = tag
                .paths
                .iter()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .take(TAG_PREVIEW_LIMIT)
                .map(|path| {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string());
                    (name, path)
                })
                .collect();
            FileTagPreview {
                tag: tag.clone(),
                preview_items,
            }
        })
        .collect()
}

pub fn load_home_file_tags() -> Vec<FileTagConfig> {
    load_config().map(|c| c.file_tags).unwrap_or_default()
}

fn path_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}
