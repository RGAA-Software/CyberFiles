use std::path::PathBuf;

use gpui::SharedString;

/// Where a tab's main content is focused (Files: path string, "Home", settings, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationTarget {
    Home,
    Path(PathBuf),
    RecycleBin,
    Settings,
    /// Files sidebar file tag: flat list of paths tagged with this name.
    FileTag(String),
}

impl NavigationTarget {
    pub fn tab_title(&self) -> SharedString {
        match self {
            NavigationTarget::Home => SharedString::from("Home"),
            NavigationTarget::RecycleBin => SharedString::from("Recycle Bin"),
            NavigationTarget::Settings => SharedString::from("Settings"),
            NavigationTarget::FileTag(name) => SharedString::from(format!("Tag: {name}")),
            NavigationTarget::Path(path) => {
                SharedString::from(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(
                    || path.to_string_lossy().to_string(),
                ))
            }
        }
    }

    pub fn toolbar_path_label(&self) -> String {
        match self {
            NavigationTarget::Home => "Home".to_string(),
            NavigationTarget::RecycleBin => "Recycle Bin".to_string(),
            NavigationTarget::Settings => "Settings".to_string(),
            NavigationTarget::FileTag(name) => name.clone(),
            NavigationTarget::Path(path) => path.to_string_lossy().to_string(),
        }
    }
}
