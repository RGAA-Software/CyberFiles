use cyberfiles_core::APP_NAME;
use gpui::{App, IntoElement, ParentElement, SharedString, Styled};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, ThemeMode, ThemeRegistry,
    group_box::GroupBoxVariant,
    label::Label,
    scroll::ScrollbarShow,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};
use rust_i18n::t;

fn ts(text: impl AsRef<str>) -> SharedString {
    SharedString::from(text.as_ref())
}

use crate::shell::preferences::{
    apply_border_radius, apply_font_size, apply_locale, apply_scrollbar_show,
    apply_theme_mode, apply_theme_name, current_locale, scrollbar_show_from_key,
    scrollbar_show_key, set_list_active_highlight,
};

pub fn build_settings(cx: &App) -> Settings {
    let theme_options: Vec<(SharedString, SharedString)> = ThemeRegistry::global(cx)
        .sorted_themes()
        .iter()
        .map(|theme| (theme.name.clone(), theme.name.clone()))
        .collect();

    let language_options = vec![
        ("en".into(), "English".into()),
        ("zh-CN".into(), "简体中文".into()),
        (crate::i18n::LOCALE_ZH_HANT.into(), "繁體中文".into()),
    ];

    let font_size_options = vec![
        ("14".into(), ts(t!("settings.font_size.small"))),
        ("16".into(), ts(t!("settings.font_size.medium"))),
        ("18".into(), ts(t!("settings.font_size.large"))),
    ];

    let radius_options = vec![
        ("0".into(), "0px".into()),
        ("4".into(), "4px".into()),
        ("6".into(), ts(t!("settings.radius.default"))),
        ("8".into(), "8px".into()),
    ];

    let scrollbar_options = vec![
        ("scrolling".into(), ts(t!("settings.scrollbar.scrolling"))),
        ("hover".into(), ts(t!("settings.scrollbar.hover"))),
        ("always".into(), ts(t!("settings.scrollbar.always"))),
    ];

    Settings::new("cyberfiles-settings")
        .with_group_variant(GroupBoxVariant::Outline)
        .pages(vec![
            SettingPage::new(ts(t!("settings.page.general")))
                .default_open(true)
                .icon(Icon::new(IconName::Settings2))
                .groups(vec![
                    SettingGroup::new()
                        .title(ts(t!("settings.group.appearance")))
                        .items(vec![
                            SettingItem::new(
                                ts(t!("settings.dark_mode")),
                                SettingField::switch(
                                    |cx: &App| cx.theme().mode.is_dark(),
                                    |enabled: bool, cx: &mut App| {
                                        let mode = if enabled {
                                            ThemeMode::Dark
                                        } else {
                                            ThemeMode::Light
                                        };
                                        apply_theme_mode(mode, cx);
                                    },
                                )
                                .default_value(cx.theme().mode.is_dark()),
                            )
                            .description(ts(t!("settings.dark_mode.description"))),
                            SettingItem::new(
                                ts(t!("settings.language")),
                                SettingField::dropdown(
                                    language_options,
                                    current_locale,
                                    |locale: SharedString, cx: &mut App| {
                                        apply_locale(locale.as_ref(), cx);
                                    },
                                )
                                .default_value(current_locale(cx)),
                            )
                            .description(ts(t!("settings.language.description"))),
                            SettingItem::new(
                                ts(t!("settings.color_theme")),
                                SettingField::scrollable_dropdown(
                                    theme_options,
                                    |cx: &App| cx.theme().theme_name().clone(),
                                    |name: SharedString, cx: &mut App| {
                                        apply_theme_name(name, cx);
                                    },
                                )
                                .default_value(cx.theme().theme_name()),
                            )
                            .description(ts(t!("settings.color_theme.description"))),
                        ]),
                    SettingGroup::new()
                        .title(ts(t!("settings.group.interface")))
                        .items(vec![
                            SettingItem::new(
                                ts(t!("settings.font_size")),
                                SettingField::dropdown(
                                    font_size_options,
                                    |cx: &App| {
                                        format!(
                                            "{}",
                                            cx.theme().font_size.as_f32().round() as i32
                                        )
                                        .into()
                                    },
                                    |val: SharedString, cx: &mut App| {
                                        if let Ok(size) = val.parse::<f32>() {
                                            apply_font_size(size, cx);
                                        }
                                    },
                                )
                                .default_value(SharedString::from(
                                    format!("{}", cx.theme().font_size.as_f32().round() as i32),
                                )),
                            )
                            .description(ts(t!("settings.font_size.description"))),
                            SettingItem::new(
                                ts(t!("settings.border_radius")),
                                SettingField::dropdown(
                                    radius_options,
                                    |cx: &App| {
                                        format!("{}", cx.theme().radius.as_f32().round() as i32)
                                            .into()
                                    },
                                    |val: SharedString, cx: &mut App| {
                                        if let Ok(radius) = val.parse::<f32>() {
                                            apply_border_radius(radius, cx);
                                        }
                                    },
                                )
                                .default_value(SharedString::from(
                                    format!("{}", cx.theme().radius.as_f32().round() as i32),
                                )),
                            )
                            .description(ts(t!("settings.border_radius.description"))),
                            SettingItem::new(
                                ts(t!("settings.scrollbar")),
                                SettingField::dropdown(
                                    scrollbar_options,
                                    |cx: &App| scrollbar_show_key(cx.theme().scrollbar_show),
                                    |val: SharedString, cx: &mut App| {
                                        apply_scrollbar_show(
                                            scrollbar_show_from_key(val.as_ref()),
                                            cx,
                                        );
                                    },
                                )
                                .default_value(scrollbar_show_key(cx.theme().scrollbar_show)),
                            )
                            .description(ts(t!("settings.scrollbar.description"))),
                            SettingItem::new(
                                ts(t!("settings.list_highlight")),
                                SettingField::switch(
                                    |cx: &App| cx.theme().list.active_highlight,
                                    |checked: bool, cx: &mut App| {
                                        set_list_active_highlight(checked, cx);
                                    },
                                )
                                .default_value(cx.theme().list.active_highlight),
                            )
                            .description(ts(t!("settings.list_highlight.description"))),
                        ]),
                ]),
            SettingPage::new(ts(t!("settings.page.about")))
                .icon(Icon::new(IconName::Info))
                .group(
                    SettingGroup::new().item(SettingItem::render(|_, _, cx| {
                        v_flex()
                            .gap_3()
                            .w_full()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::GalleryVerticalEnd).size_16())
                            .child(APP_NAME)
                            .child(
                                Label::new(ts(t!(
                                    "settings.about.description",
                                    app = APP_NAME
                                )))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                            )
                    })),
                ),
        ])
}
