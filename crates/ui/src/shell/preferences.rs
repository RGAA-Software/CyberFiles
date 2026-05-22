use cyberfiles_core::{self, AppConfig, WINDOW_HEIGHT, WINDOW_WIDTH, save_config};
use gpui::{App, SharedString, px};
use gpui_component::{ActiveTheme as _, Theme, ThemeMode, scroll::ScrollbarShow};

use crate::theme::{self, ThemeCatalog};

use crate::i18n;

fn persist_preferences(cx: &mut App) {
    let (window_width, window_height) =
        window_size_from_active(cx).unwrap_or_else(|| {
            cyberfiles_core::load_config()
                .map(|c| (c.window_width, c.window_height))
                .unwrap_or((WINDOW_WIDTH, WINDOW_HEIGHT))
        });
    let _ = save_config(&capture_config(cx, window_width, window_height));
}

/// Reads the active window size in pixels (for persistence).
pub fn window_size_from_active(cx: &mut App) -> Option<(f32, f32)> {
    let window = cx.active_window()?;
    let mut size = None;
    let _ = window.update(cx, |_, window, _| {
        let bounds = window.window_bounds().get_bounds();
        size = Some((bounds.size.width.as_f32(), bounds.size.height.as_f32()));
    });
    size
}

/// Saves current window bounds into `settings.json` (theme fields unchanged).
pub fn persist_window_bounds(cx: &mut App) {
    let Some((window_width, window_height)) = window_size_from_active(cx) else {
        return;
    };
    let _ = save_config(&capture_config(cx, window_width, window_height));
}

pub fn apply_locale(locale: &str, cx: &mut App) {
    i18n::set_locale(locale);
    super::app_menus::reload(cx);
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn apply_theme_mode(mode: ThemeMode, cx: &mut App) {
    let set_id = theme::current_theme_set_id(cx);
    theme::apply_set(set_id.as_ref(), mode, cx);
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn apply_theme_name(name: SharedString, cx: &mut App) {
    let mode = Theme::global(cx).mode;
    theme::apply_set(name.as_ref(), mode, cx);
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn apply_font_size(size: f32, cx: &mut App) {
    Theme::global_mut(cx).font_size = px(size);
    cx.refresh_windows();
    persist_preferences(cx);
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
    persist_preferences(cx);
}

pub fn apply_scrollbar_show(show: ScrollbarShow, cx: &mut App) {
    Theme::global_mut(cx).scrollbar_show = show;
    cx.refresh_windows();
    persist_preferences(cx);
}

pub fn set_list_active_highlight(enabled: bool, cx: &mut App) {
    Theme::global_mut(cx).list.active_highlight = enabled;
    cx.refresh_windows();
    persist_preferences(cx);
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

fn mutate_config(cx: &mut App, mutate: impl FnOnce(&mut AppConfig)) {
    let mut config = cyberfiles_core::load_config().unwrap_or_default();
    mutate(&mut config);
    let _ = save_config(&config);
    refresh_main_page_sidebar(cx);
    cx.refresh_windows();
}

fn refresh_main_page_sidebar(cx: &mut App) {
    let Some(nav) = cx.try_global::<crate::app_state::AppNavigation>() else {
        return;
    };
    let page = nav.main_page();
    let _ = page.update(cx, |page, cx| page.refresh_sidebar_cache(cx));
}

pub fn sidebar_display_mode(_cx: &App) -> SharedString {
    cyberfiles_core::load_config()
        .map(|c| c.sidebar_display_mode.into())
        .unwrap_or_else(|| "expanded".into())
}

pub fn apply_sidebar_display_mode(mode: SharedString, cx: &mut App) {
    mutate_config(cx, |config| {
        config.sidebar_display_mode = mode.to_string();
    });
}

pub fn sidebar_section_pinned(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_sidebar_section_pinned)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_pinned(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_pinned = enabled);
}

pub fn sidebar_section_library(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_sidebar_section_library)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_library(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_library = enabled);
}

pub fn sidebar_section_drives(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_sidebar_section_drives)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_drives(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_drives = enabled);
}

pub fn sidebar_section_cloud(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_sidebar_section_cloud)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_cloud(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_cloud = enabled);
}

pub fn sidebar_section_network(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_sidebar_section_network)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_network(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_network = enabled);
}

pub fn sidebar_section_wsl(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_sidebar_section_wsl)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_wsl(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_wsl = enabled);
}

pub fn sidebar_section_file_tags(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_sidebar_section_file_tags)
        .unwrap_or(true)
}

pub fn apply_sidebar_section_file_tags(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_sidebar_section_file_tags = enabled);
}

pub fn home_widget_quick_access(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_home_quick_access)
        .unwrap_or(true)
}

pub fn apply_home_widget_quick_access(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_quick_access = enabled);
    refresh_home_if_active(cx);
}

pub fn home_widget_drives(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_home_drives)
        .unwrap_or(true)
}

pub fn apply_home_widget_drives(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_drives = enabled);
    refresh_home_if_active(cx);
}

pub fn home_widget_network(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_home_network)
        .unwrap_or(true)
}

pub fn apply_home_widget_network(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_network = enabled);
    refresh_home_if_active(cx);
}

pub fn home_widget_file_tags(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_home_file_tags)
        .unwrap_or(true)
}

pub fn apply_home_widget_file_tags(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_file_tags = enabled);
    refresh_home_if_active(cx);
}

pub fn home_widget_recent(_cx: &App) -> bool {
    cyberfiles_core::load_config()
        .map(|c| c.show_home_recent)
        .unwrap_or(true)
}

pub fn apply_home_widget_recent(enabled: bool, cx: &mut App) {
    mutate_config(cx, |c| c.show_home_recent = enabled);
    refresh_home_if_active(cx);
}

fn refresh_home_if_active(cx: &mut App) {
    if let Some(nav) = cx.try_global::<crate::app_state::AppNavigation>() {
        let page = nav.main_page();
        let _ = page.update(cx, |page, cx| page.refresh_home_widgets(cx));
    }
}

pub fn scrollbar_show_from_key(key: &str) -> ScrollbarShow {
    match key {
        "hover" => ScrollbarShow::Hover,
        "always" => ScrollbarShow::Always,
        _ => ScrollbarShow::Scrolling,
    }
}

/// Apply saved settings at startup (before the window and app menus exist).
pub fn apply_config(config: &AppConfig, cx: &mut App) {
    i18n::set_locale(&config.locale);
    let mode = if config.dark_mode {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    let set_id = ThemeCatalog::normalize_set_id(&config.theme_name);
    theme::apply_set(set_id.as_ref(), mode, cx);
    Theme::global_mut(cx).font_size = px(config.font_size);
    let theme = Theme::global_mut(cx);
    theme.radius = px(config.border_radius);
    theme.radius_lg = if theme.radius > px(0.) {
        theme.radius + px(2.)
    } else {
        px(0.)
    };
    theme.scrollbar_show = scrollbar_show_from_key(&config.scrollbar_show);
    theme.list.active_highlight = config.list_active_highlight;
}

pub fn capture_config(cx: &App, window_width: f32, window_height: f32) -> AppConfig {
    let prior = cyberfiles_core::load_config().unwrap_or_default();
    AppConfig {
        locale: i18n::locale().to_string(),
        dark_mode: cx.theme().mode.is_dark(),
        theme_name: theme::current_theme_set_id(cx).to_string(),
        font_size: cx.theme().font_size.as_f32(),
        border_radius: cx.theme().radius.as_f32(),
        scrollbar_show: scrollbar_show_key(cx.theme().scrollbar_show).to_string(),
        list_active_highlight: cx.theme().list.active_highlight,
        window_width,
        window_height,
        pinned_folders: prior.pinned_folders,
        show_info_pane: prior.show_info_pane,
        file_view_mode: prior.file_view_mode,
        file_sort_option: prior.file_sort_option,
        file_sort_direction: prior.file_sort_direction,
        file_show_hidden: prior.file_show_hidden,
        path_history: prior.path_history,
        sidebar_display_mode: prior.sidebar_display_mode,
        sidebar_collapsed: prior.sidebar_collapsed,
        show_sidebar_section_pinned: prior.show_sidebar_section_pinned,
        show_sidebar_section_library: prior.show_sidebar_section_library,
        show_sidebar_section_drives: prior.show_sidebar_section_drives,
        show_sidebar_section_cloud: prior.show_sidebar_section_cloud,
        show_sidebar_section_network: prior.show_sidebar_section_network,
        show_sidebar_section_wsl: prior.show_sidebar_section_wsl,
        show_sidebar_section_file_tags: prior.show_sidebar_section_file_tags,
        file_tags: prior.file_tags,
        show_home_quick_access: prior.show_home_quick_access,
        show_home_drives: prior.show_home_drives,
        show_home_network: prior.show_home_network,
        show_home_file_tags: prior.show_home_file_tags,
        show_home_recent: prior.show_home_recent,
        home_quick_access_expanded: prior.home_quick_access_expanded,
        home_drives_expanded: prior.home_drives_expanded,
        home_network_expanded: prior.home_network_expanded,
        home_file_tags_expanded: prior.home_file_tags_expanded,
        home_recent_expanded: prior.home_recent_expanded,
    }
}
