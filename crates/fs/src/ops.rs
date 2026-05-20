use std::path::{Path, PathBuf};

pub fn create_directory(parent: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("folder name cannot be empty");
    }

    let path = parent.join(trimmed);
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }

    std::fs::create_dir(&path)?;
    Ok(path)
}

pub fn rename_path(path: &Path, new_name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("name cannot be empty");
    }

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot rename {}", path.display()))?;
    let target = parent.join(trimmed);

    if target == path {
        return Ok(target);
    }

    if target.exists() {
        anyhow::bail!("{} already exists", target.display());
    }

    std::fs::rename(path, &target)?;
    Ok(target)
}

pub fn delete_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    for path in paths {
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else if path.is_symlink() || path.is_file() {
            std::fs::remove_file(path)?;
        } else if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub fn unique_new_folder_name(parent: &Path) -> String {
    let base = "New folder";
    let mut candidate = base.to_string();
    let mut counter = 2;

    while parent.join(&candidate).exists() {
        candidate = format!("{base} ({counter})");
        counter += 1;
    }

    candidate
}
