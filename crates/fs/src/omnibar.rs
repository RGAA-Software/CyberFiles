use std::path::{Path, PathBuf};

/// One clickable segment in the omnibar breadcrumb trail.
#[derive(Debug, Clone)]
pub struct PathBreadcrumb {
    pub label: String,
    pub path: PathBuf,
}

/// Builds path segments for breadcrumb UI (e.g. `C:\` → `Users` → `hy`).
pub fn path_breadcrumbs(path: &Path) -> Vec<PathBreadcrumb> {
    let mut segments: Vec<PathBuf> = path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect();
    segments.reverse();

    if segments.is_empty() && !path.as_os_str().is_empty() {
        segments.push(path.to_path_buf());
    }

    segments
        .into_iter()
        .map(|segment| PathBreadcrumb {
            label: breadcrumb_label(&segment),
            path: segment,
        })
        .collect()
}

fn breadcrumb_label(path: &Path) -> String {
    let text = path.to_string_lossy();
    #[cfg(windows)]
    {
        let bytes = text.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes.len() <= 3 && bytes.last() == Some(&b'\\') {
            return text.to_string();
        }
    }
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| text.to_string())
}

/// Suggestion row for omnibar autocomplete.
#[derive(Debug, Clone)]
pub struct OmnibarPathSuggestion {
    pub path: PathBuf,
    pub label: String,
}

const MAX_SUGGESTIONS: usize = 10;
const MAX_BREADCRUMB_ENTRIES: usize = 50;

/// Subfolder entries for a breadcrumb segment dropdown (chevron menu).
pub fn breadcrumb_dropdown_entries(path: &Path) -> Vec<OmnibarPathSuggestion> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };

    let mut suggestions = Vec::new();
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }
        suggestions.push(OmnibarPathSuggestion {
            label: entry.file_name().to_string_lossy().to_string(),
            path: entry_path,
        });
        if suggestions.len() >= MAX_BREADCRUMB_ENTRIES {
            break;
        }
    }

    suggestions.sort_by(|a, b| a.label.cmp(&b.label));
    suggestions
}

/// Path-mode suggestions: history when input is empty/special, otherwise child folder names.
pub fn omnibar_path_suggestions(
    query: &str,
    path_history: &[String],
) -> Vec<OmnibarPathSuggestion> {
    let trimmed = query.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("home")
        || trimmed.eq_ignore_ascii_case("settings")
        || trimmed.eq_ignore_ascii_case("recycle bin")
    {
        return path_history
            .iter()
            .take(MAX_SUGGESTIONS)
            .map(|entry| {
                let path = PathBuf::from(entry);
                OmnibarPathSuggestion {
                    label: entry.clone(),
                    path,
                }
            })
            .collect();
    }

    let (parent, partial) = parent_and_partial(trimmed);
    let parent_path = PathBuf::from(&parent);
    if !parent_path.is_dir() {
        return Vec::new();
    }

    let partial_lower = partial.to_ascii_lowercase();
    let mut suggestions = Vec::new();

    let Ok(entries) = std::fs::read_dir(&parent_path) else {
        return suggestions;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !partial.is_empty() && !name.to_ascii_lowercase().starts_with(&partial_lower) {
            continue;
        }
        suggestions.push(OmnibarPathSuggestion {
            label: path.to_string_lossy().to_string(),
            path,
        });
        if suggestions.len() >= MAX_SUGGESTIONS {
            break;
        }
    }

    suggestions.sort_by(|a, b| a.label.cmp(&b.label));
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn path_breadcrumbs_windows_drive_root() {
        let crumbs = path_breadcrumbs(Path::new(r"C:\"));
        assert_eq!(crumbs.len(), 1);
        assert_eq!(crumbs[0].label, "C:\\");
        assert_eq!(crumbs[0].path, Path::new(r"C:\"));
    }

    #[test]
    fn path_breadcrumbs_nested_path() {
        let crumbs = path_breadcrumbs(Path::new(r"C:\Users\hy"));
        assert_eq!(crumbs.len(), 3);
        assert_eq!(crumbs[0].path, Path::new(r"C:\"));
        assert_eq!(crumbs[1].path, Path::new(r"C:\Users"));
        assert_eq!(crumbs[2].path, Path::new(r"C:\Users\hy"));
    }
}

fn parent_and_partial(input: &str) -> (String, String) {
    let path = Path::new(input);
    let parent = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| input.to_string());
    let partial = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    (parent, partial)
}
