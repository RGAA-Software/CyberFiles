pub mod config;
pub mod path_history;

pub const APP_NAME: &str = "CyberFiles";

pub const WINDOW_WIDTH: f32 = 1366.;
pub const WINDOW_HEIGHT: f32 = 768.;

pub use config::{
    file_sort_prefs_from_config, file_view_mode_from_config, pinned_folder_paths,
    save_file_browser_prefs, AppConfig, load_config, save_config, window_size, VIEW_COLUMNS,
    VIEW_DETAILS, VIEW_GRID,
};
pub use path_history::{path_history_list, record_path_history};
