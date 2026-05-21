//! Shell Frequent-places folder — Explorer Quick Access pins.

use crate::shell_folder::{list_known_folder_folders, ShellFolderEntry, FOLDERID_FREQUENT};

/// One pinned folder from the Shell Quick Access / Frequent list.
pub type ShellQuickAccessEntry = ShellFolderEntry;

/// Lists folders pinned to Windows Quick Access (Frequent places known folder).
pub fn list_shell_quick_access_folders() -> anyhow::Result<Vec<ShellQuickAccessEntry>> {
    list_known_folder_folders(&FOLDERID_FREQUENT)
}
