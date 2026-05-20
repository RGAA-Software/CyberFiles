use gpui::{App, SharedString, px};
use gpui_component::{Theme, ThemeMode, ThemeRegistry, scroll::ScrollbarShow};

use crate::i18n;

pub fn apply_locale(locale: &str, cx: &mut App) {
    i18n::set_locale(locale);
    super::app_menus::reload(cx);
    cx.refresh_windows();
}

pub fn apply_theme_mode(mode: ThemeMode, cx: &mut App) {
    Theme::change(mode, None, cx);
    cx.refresh_windows();
}

pub fn apply_theme_name(name: SharedString, cx: &mut App) {
    if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(name.as_ref()).cloned() {
        Theme::global_mut(cx).apply_config(&theme_config);
    }
    cx.refresh_windows();
}

pub fn apply_font_size(size: f32, cx: &mut App) {
    Theme::global_mut(cx).font_size = px(size);
    cx.refresh_windows();
}

pub fn apply_border_radius(radius: f32, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.radius = px(radius);
    theme.radius_lg = if theme.radius > px(0.) {
        theme.radius + px(2.)
    } else {
        px(0.)
    };
    cx.refresh_windows();
}

pub fn apply_scrollbar_show(show: ScrollbarShow, cx: &mut App) {
    Theme::global_mut(cx).scrollbar_show = show;
    cx.refresh_windows();
}

pub fn set_list_active_highlight(enabled: bool, cx: &mut App) {
    Theme::global_mut(cx).list.active_highlight = enabled;
    cx.refresh_windows();
}

pub fn current_locale(_cx: &App) -> SharedString {
    i18n::locale().to_string().into()
}

pub fn scrollbar_show_key(show: ScrollbarShow) -> SharedString {
    match show {
        ScrollbarShow::Scrolling => "scrolling".into(),
        ScrollbarShow::Hover => "hover".into(),
        ScrollbarShow::Always => "always".into(),
    }
}

pub fn scrollbar_show_from_key(key: &str) -> ScrollbarShow {
    match key {
        "hover" => ScrollbarShow::Hover,
        "always" => ScrollbarShow::Always,
        _ => ScrollbarShow::Scrolling,
    }
}
