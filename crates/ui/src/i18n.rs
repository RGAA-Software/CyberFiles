use std::ops::Deref;

use rust_i18n::t;

/// Detect system locale and apply `en` or `zh-CN` for the app and gpui-component.
pub fn init_locale() {
    let locale = detect_locale();
    set_locale(&locale);
}

pub fn locale() -> impl Deref<Target = str> {
    rust_i18n::locale()
}

/// Switch app and gpui-component UI strings (calendar, dialogs, etc.).
pub fn set_locale(locale: &str) {
    let locale = normalize_locale(locale);
    rust_i18n::set_locale(locale);
    gpui_component::set_locale(locale);
}

fn detect_locale() -> String {
    sys_locale::get_locale()
        .map(|l| normalize_locale(&l).to_string())
        .unwrap_or_else(|| "en".to_string())
}

fn normalize_locale(locale: &str) -> &'static str {
    if locale.starts_with("zh") {
        "zh-CN"
    } else {
        "en"
    }
}

pub fn nav_name(id: &str) -> String {
    match id {
        "home" => t!("nav.home").to_string(),
        "files" => t!("nav.files").to_string(),
        "settings" => t!("nav.settings").to_string(),
        other => other.to_string(),
    }
}

pub fn nav_description(id: &str) -> String {
    match id {
        "home" => t!("nav.home.description").to_string(),
        "files" => t!("nav.files.description").to_string(),
        "settings" => t!("nav.settings.description").to_string(),
        other => other.to_string(),
    }
}
