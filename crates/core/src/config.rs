use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::{APP_NAME, WINDOW_HEIGHT, WINDOW_WIDTH};

/// Persisted file browser view: `details`, `grid`, or `columns`.
pub const VIEW_DETAILS: &str = "details";
pub const VIEW_GRID: &str = "grid";
pub const VIEW_COLUMNS: &str = "columns";

const CONFIG_FILE: &str = "settings.json";

/// Persisted user preferences (written on save, applied on next launch).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub locale: String,
    pub dark_mode: bool,
    pub theme_name: String,
    pub font_size: f32,
    pub border_radius: f32,
    pub scrollbar_show: String,
    pub list_active_highlight: bool,
    pub window_width: f32,
    pub window_height: f32,
    #[serde(default)]
    pub pinned_folders: Vec<String>,
    #[serde(default = "default_show_info_pane")]
    pub show_info_pane: bool,
    #[serde(default = "default_file_view_mode")]
    pub file_view_mode: String,
    #[serde(default)]
    pub file_sort_option: Option<String>,
    #[serde(default)]
    pub file_sort_direction: Option<String>,
    #[serde(default)]
    pub file_show_hidden: Option<bool>,
    /// Recently navigated paths from the omnibar (Files `PathHistoryList`).
    #[serde(default)]
    pub path_history: Vec<String>,
    /// Sidebar width mode: `expanded`, `compact` (icon), `minimal` (offcanvas).
    #[serde(default = "default_sidebar_display_mode")]
    pub sidebar_display_mode: String,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_pinned: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_library: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_drives: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_cloud: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_network: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_wsl: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_file_tags: bool,
    /// User-defined file tags for sidebar (full tag system is future work).
    #[serde(default)]
    pub file_tags: Vec<FileTagConfig>,
}

/// Sidebar file tag entry (Files `FileTagsManager` subset).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTagConfig {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

fn default_sidebar_display_mode() -> String {
    "expanded".into()
}

fn default_true() -> bool {
    true
}

fn default_show_info_pane() -> bool {
    true
}

fn default_file_view_mode() -> String {
    VIEW_DETAILS.into()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            dark_mode: false,
            theme_name: "Default".into(),
            font_size: 16.,
            border_radius: 6.,
            scrollbar_show: "scrolling".into(),
            list_active_highlight: false,
            window_width: WINDOW_WIDTH,
            window_height: WINDOW_HEIGHT,
            pinned_folders: Vec::new(),
            show_info_pane: true,
            file_view_mode: default_file_view_mode(),
            file_sort_option: None,
            file_sort_direction: None,
            file_show_hidden: None,
            path_history: Vec::new(),
            sidebar_display_mode: default_sidebar_display_mode(),
            sidebar_collapsed: false,
            show_sidebar_section_pinned: true,
            show_sidebar_section_library: true,
            show_sidebar_section_drives: true,
            show_sidebar_section_cloud: true,
            show_sidebar_section_network: true,
            show_sidebar_section_wsl: true,
            show_sidebar_section_file_tags: true,
            file_tags: Vec::new(),
        }
    }
}

/// Sidebar is icon-only when `sidebar_display_mode == "compact"`.
pub fn sidebar_is_compact(config: &AppConfig) -> bool {
    config.sidebar_display_mode == "compact"
}

pub fn sidebar_is_offcanvas(config: &AppConfig) -> bool {
    config.sidebar_display_mode == "minimal"
}

/// Updates file-browser fields in settings and writes `settings.json`.
pub fn save_file_browser_prefs(
    view_mode: &str,
    sort_option: &str,
    sort_direction: &str,
    show_hidden: bool,
) -> anyhow::Result<()> {
    let mut config = load_config().unwrap_or_default();
    config.file_view_mode = view_mode.to_string();
    config.file_sort_option = Some(sort_option.to_string());
    config.file_sort_direction = Some(sort_direction.to_string());
    config.file_show_hidden = Some(show_hidden);
    save_config(&config)
}

pub fn file_view_mode_from_config() -> String {
    load_config()
        .map(|c| c.file_view_mode)
        .unwrap_or_else(default_file_view_mode)
}

pub fn file_sort_prefs_from_config() -> (Option<String>, Option<String>, Option<bool>) {
    load_config().map(|c| (c.file_sort_option, c.file_sort_direction, c.file_show_hidden))
        .unwrap_or((None, None, None))
}

pub fn pinned_folder_paths() -> Vec<PathBuf> {
    load_config()
        .map(|c| {
            c.pinned_folders
                .into_iter()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}

pub fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "cyberfiles", APP_NAME).map(|dirs| dirs.config_dir().join(CONFIG_FILE))
}

pub fn load_config() -> Option<AppConfig> {
    let path = config_path()?;
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save_config(config: &AppConfig) -> anyhow::Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("config directory unavailable"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn window_size() -> (f32, f32) {
    load_config()
        .map(|c| (c.window_width, c.window_height))
        .unwrap_or((WINDOW_WIDTH, WINDOW_HEIGHT))
}
