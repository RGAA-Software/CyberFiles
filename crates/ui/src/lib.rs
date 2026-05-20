mod app_view;
mod i18n;
mod shell;

rust_i18n::i18n!("locales", fallback = "en");

use gpui::App;
use gpui_component::{Theme, ThemeRegistry};

pub use app_view::AppView;
pub use gpui_component_assets::Assets;
pub use i18n::{init_locale, locale, set_locale};
pub use shell::open_main_window;

pub fn init(cx: &mut App) {
    init_locale();
    gpui_component::init(cx);

    cx.on_action(|_: &shell::Quit, cx| {
        cx.quit();
    });

    cx.on_action(|switch: &shell::SwitchTheme, cx| {
        let theme_name = switch.0.clone();
        if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme_config);
        }
        cx.refresh_windows();
    });

    cx.on_action(|switch: &shell::SwitchThemeMode, cx| {
        Theme::change(switch.0, None, cx);
        cx.refresh_windows();
    });
}
