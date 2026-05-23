pub mod config;
pub mod path_history;

pub const APP_NAME: &str = "CyberFiles";

pub const WINDOW_WIDTH: f32 = 1440.;
pub const WINDOW_HEIGHT: f32 = 900.;

pub use config::{
    file_sort_prefs_from_config, file_view_mode_from_config, home_widget_prefs,
    pinned_folder_paths, save_file_browser_prefs, save_home_widget_prefs, sidebar_is_compact,
    context_menu_item_prefs, default_home_widget_order, flush_config,
    normalize_home_widget_order, sidebar_is_offcanvas, AppConfig, ContextMenuItemPrefs,
    FileTagConfig, HomeWidgetPrefs, load_config,
    ClosedTabSession, SessionPaneLayout,
    save_config,
    window_size, VIEW_COLUMNS, VIEW_DETAILS, VIEW_GRID,
};
pub use path_history::{path_history_list, record_path_history};
