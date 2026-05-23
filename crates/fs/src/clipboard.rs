use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOperation {
    Copy,
    Cut,
}

/// User choice when a destination path already exists (Files conflict dialog subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    Skip,
    Replace,
    SkipAll,
    ReplaceAll,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct TransferConflict {
    pub source: PathBuf,
    pub target: PathBuf,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransferOutcome {
    pub transferred: u32,
    pub cancelled: bool,
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
        transfer_one(source, destination_dir, ClipboardOperation::Copy, false)?;
    }
    Ok(())
}

pub fn move_items(sources: &[PathBuf], destination_dir: &Path) -> anyhow::Result<()> {
    for source in sources {
        transfer_one(source, destination_dir, ClipboardOperation::Cut, false)?;
    }
    Ok(())
}

/// Copy or move `sources` into `destination_dir`, prompting via `resolve` on name collisions.
pub fn transfer_items(
    sources: &[PathBuf],
    destination_dir: &Path,
    operation: ClipboardOperation,
    resolve: &mut dyn FnMut(TransferConflict) -> ConflictResolution,
) -> anyhow::Result<TransferOutcome> {
    let mut skip_all = false;
    let mut replace_all = false;
    let mut outcome = TransferOutcome::default();

    for source in sources {
        let file_name = source
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid source path {}", source.display()))?;
        let target = destination_dir.join(file_name);

        if target.exists() && !same_path(source, &target) {
            if skip_all {
                continue;
            }
            if !replace_all {
                match resolve(TransferConflict {
                    source: source.clone(),
                    target: target.clone(),
                }) {
                    ConflictResolution::Skip => continue,
                    ConflictResolution::SkipAll => {
                        skip_all = true;
                        continue;
                    }
                    ConflictResolution::Replace => {}
                    ConflictResolution::ReplaceAll => replace_all = true,
                    ConflictResolution::Cancel => {
                        outcome.cancelled = true;
                        return Ok(outcome);
                    }
                }
            }
        }

        let must_replace = target.exists() && !same_path(source, &target);
        transfer_one(source, destination_dir, operation, must_replace)?;
        outcome.transferred += 1;
    }

    Ok(outcome)
}

pub fn transfer_one(
    source: &Path,
    destination_dir: &Path,
    operation: ClipboardOperation,
    replace_existing: bool,
) -> anyhow::Result<()> {
    let file_name = source
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("invalid source path {}", source.display()))?;
    let target = destination_dir.join(file_name);

    if target.exists() {
        if same_path(source, &target) {
            return Ok(());
        }
        if !replace_existing {
            anyhow::bail!("{} already exists", target.display());
        }
        remove_path_recursive(&target)?;
    }

    match operation {
        ClipboardOperation::Copy => copy_path_recursive(source, &target),
        ClipboardOperation::Cut => {
            if std::fs::rename(source, &target).is_err() {
                copy_path_recursive(source, &target)?;
                remove_path_recursive(source)?;
            }
            Ok(())
        }
    }
}

pub fn paths_conflict(source: &Path, target: &Path) -> bool {
    target.exists() && !same_path(source, target)
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
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
