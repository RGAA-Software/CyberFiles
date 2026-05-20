//! Windows-only Shell helpers (icons, clipboard file lists, known folders).

#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
mod context_menu;
#[cfg(windows)]
mod icons;
#[cfg(windows)]
mod paths;
#[cfg(windows)]
mod recycle;
#[cfg(windows)]
mod shell;

#[cfg(windows)]
pub use clipboard::read_clipboard_file_paths;
#[cfg(windows)]
pub use icons::{icon_hint_for_path, ShellIconHint};
#[cfg(windows)]
pub use paths::{is_recycle_bin_path, recycle_bin_folder};
#[cfg(windows)]
pub use recycle::{list_recycle_bin_entries, RecycleBinEntry};
#[cfg(windows)]
pub use shell::{open_item_properties, show_shell_context_menu};

#[cfg(not(windows))]
pub use stubs::*;

#[cfg(not(windows))]
mod stubs {
    use std::path::{Path, PathBuf};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ShellIconHint {
        Folder,
        File,
        Symlink,
        Executable,
        Image,
        Archive,
    }

    pub fn icon_hint_for_path(_path: &Path) -> ShellIconHint {
        ShellIconHint::File
    }

    pub fn recycle_bin_folder() -> Option<PathBuf> {
        None
    }

    pub fn is_recycle_bin_path(_path: &Path) -> bool {
        false
    }

    pub fn read_clipboard_file_paths() -> Vec<PathBuf> {
        Vec::new()
    }

    pub fn show_shell_context_menu(_paths: &[PathBuf]) -> anyhow::Result<()> {
        Ok(())
    }

    #[derive(Debug, Clone)]
    pub struct RecycleBinEntry {
        pub display_name: String,
        pub shell_path: PathBuf,
        pub size: Option<u64>,
        pub modified: Option<std::time::SystemTime>,
    }

    pub fn list_recycle_bin_entries() -> Vec<RecycleBinEntry> {
        Vec::new()
    }

    pub fn open_item_properties(_path: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
