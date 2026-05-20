mod app_view;
mod file_browser;
mod files_shell;
mod i18n;
mod settings_view;
mod shell;

rust_i18n::i18n!("locales", fallback = "en");

use gpui::App;

pub use app_view::AppView;
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
    if let Some(ref cfg) = config {
        shell::preferences::apply_config(cfg, cx);
    }

    cx.on_action(|_: &shell::Quit, cx| {
        cx.quit();
    });

}
