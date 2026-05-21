mod app_state;
mod drag;
mod file_browser;
mod home;
mod i18n;
mod info_pane;
mod main_page;
mod omnibar;
mod settings_view;
mod shell;
mod sidebar;

rust_i18n::i18n!("locales", fallback = "en");

use gpui::App;

pub use main_page::MainPage;
pub use gpui_component_assets::Assets;
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
    cx.set_global(crate::app_state::AppFileClipboard::default());
    if let Some(ref cfg) = config {
        shell::preferences::apply_config(cfg, cx);
    }

    cx.on_action(|_: &shell::Quit, cx| {
        shell::preferences::persist_window_bounds(cx);
        cx.quit();
    });

}
