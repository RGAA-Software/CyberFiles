//! Windows-only Shell helpers (icons, clipboard file lists, known folders).

#[cfg(windows)]
mod com;
#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
mod context_menu;
#[cfg(windows)]
mod icons;
#[cfg(windows)]
mod paths;
#[cfg(windows)]
mod quick_access;
#[cfg(windows)]
mod shell_folder;
#[cfg(windows)]
mod recycle;
#[cfg(windows)]
mod shell;
#[cfg(windows)]
mod volume;

#[cfg(windows)]
pub use clipboard::read_clipboard_file_paths;
#[cfg(windows)]
pub use icons::{icon_hint_for_path, ShellIconHint};
#[cfg(windows)]
pub use paths::{is_recycle_bin_path, recycle_bin_folder};
#[cfg(windows)]
pub use quick_access::{list_shell_quick_access_folders, ShellQuickAccessEntry};
#[cfg(windows)]
pub use shell_folder::{
    list_cloud_drive_roots, list_known_folder_folders, list_wsl_distro_roots, ShellFolderEntry,
    FOLDERID_LIBRARIES, FOLDERID_NETWORK,
};
#[cfg(windows)]
pub use recycle::{list_recycle_bin_entries, RecycleBinEntry};
#[cfg(windows)]
pub use volume::{DriveKind, VolumeDetails, volume_details};
#[cfg(windows)]
pub use shell::{
    invoke_shell_context_menu_item, open_item_properties, query_shell_context_menu_items,
    show_shell_context_menu, ShellContextMenuEntry,
};

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
    pub enum ShellContextMenuEntry {
        Separator,
        Item {
            label: String,
            command_offset: u32,
            command_string: Option<String>,
        },
    }

    pub fn query_shell_context_menu_items(
        _paths: &[PathBuf],
        _extended_verbs: bool,
    ) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
        Ok(Vec::new())
    }

    pub fn invoke_shell_context_menu_item(
        _paths: &[PathBuf],
        _command_offset: u32,
    ) -> anyhow::Result<()> {
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

    #[derive(Debug, Clone)]
    pub struct ShellQuickAccessEntry {
        pub display_name: String,
        pub path: PathBuf,
    }

    pub fn list_shell_quick_access_folders() -> anyhow::Result<Vec<ShellQuickAccessEntry>> {
        Ok(Vec::new())
    }

    #[derive(Debug, Clone)]
    pub struct ShellFolderEntry {
        pub display_name: String,
        pub path: PathBuf,
    }


    pub fn open_item_properties(_path: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}
