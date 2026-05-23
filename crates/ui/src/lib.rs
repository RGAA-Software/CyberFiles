mod app_state;
mod drag;
mod file_ops;
mod file_browser;
mod home;
mod icons;
mod i18n;
mod info_pane;
mod main_page;
mod omnibar;
mod settings_view;
mod shell;
mod list_icon_cache;
mod shell_icon;
mod popup_menu;
mod sidebar;
mod theme;
mod toolbar_button;

rust_i18n::i18n!("locales", fallback = "en");

use gpui::App;

pub use main_page::MainPage;
pub use popup_menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};
pub use cyberfiles_assets::Assets;
pub use i18n::{init_locale, locale, set_locale};
pub use shell::open_main_window;

pub fn init(cx: &mut App) {
    cyberfiles_commands::init(cx);

    let config = cyberfiles_core::load_config();
    if let Some(ref cfg) = config {
        set_locale(&cfg.locale);
    } else {
        init_locale();
    }
    gpui_component::init(cx);
    popup_menu::init(cx);
    theme::install(cx);
    cx.set_global(crate::app_state::AppFileClipboard::default());
    if let Some(ref cfg) = config {
        shell::preferences::apply_config(cfg, cx);
    }

    #[cfg(windows)]
    cyberfiles_platform_windows::warm_up_query_context_menu();

    cx.on_action(|_: &shell::Quit, cx| {
        shell::preferences::persist_window_bounds(cx);
        cx.quit();
    });

}
