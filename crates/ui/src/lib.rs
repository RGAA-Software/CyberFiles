mod app_view;
mod shell;

use gpui::App;
use gpui_component::{Theme, ThemeRegistry};

pub use app_view::AppView;
pub use gpui_component_assets::Assets;
pub use shell::open_main_window;

pub fn init(cx: &mut App) {
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
