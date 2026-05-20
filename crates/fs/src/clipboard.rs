use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOperation {
    Copy,
    Cut,
}

#[derive(Debug, Clone)]
pub struct FileClipboard {
    pub operation: ClipboardOperation,
    pub paths: Vec<PathBuf>,
}

impl FileClipboard {
    pub fn new(operation: ClipboardOperation, paths: Vec<PathBuf>) -> Self {
        Self { operation, paths }
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

pub fn copy_items(sources: &[PathBuf], destination_dir: &Path) -> anyhow::Result<()> {
    for source in sources {
        let file_name = source
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid source path {}", source.display()))?;
        let target = destination_dir.join(file_name);
        if target.exists() {
            anyhow::bail!("{} already exists", target.display());
        }
        copy_path_recursive(source, &target)?;
    }
    Ok(())
}

pub fn move_items(sources: &[PathBuf], destination_dir: &Path) -> anyhow::Result<()> {
    for source in sources {
        let file_name = source
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid source path {}", source.display()))?;
        let target = destination_dir.join(file_name);
        if target.exists() {
            anyhow::bail!("{} already exists", target.display());
        }
        if std::fs::rename(source, &target).is_err() {
            copy_path_recursive(source, &target)?;
            remove_path_recursive(source)?;
        }
    }
    Ok(())
}

pub fn remove_path_recursive(path: &Path) -> anyhow::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn copy_path_recursive(source: &Path, target: &Path) -> anyhow::Result<()> {
    if source.is_dir() {
        std::fs::create_dir_all(target)?;
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name();
            copy_path_recursive(&entry.path(), &target.join(name))?;
        }
    } else {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(source, target)?;
    }
    Ok(())
}
