use gpui::{App, Entity, Menu, MenuItem, SharedString};
use gpui_component::{
    ActiveTheme as _, GlobalState, Theme, ThemeMode, ThemeRegistry, menu::AppMenuBar,
};

use crate::i18n;
use rust_i18n::t;

use super::actions::{About, Quit, SelectLocale, SwitchTheme, SwitchThemeMode};

pub fn init(title: impl Into<SharedString>, cx: &mut App) -> Entity<AppMenuBar> {
    let app_menu_bar = AppMenuBar::new(cx);
    let title: SharedString = title.into();
    update_app_menu(title.clone(), app_menu_bar.clone(), cx);

    cx.on_action({
        let title = title.clone();
        let app_menu_bar = app_menu_bar.clone();
        move |select: &SelectLocale, cx: &mut App| {
            i18n::set_locale(&select.0);
            update_app_menu(title.clone(), app_menu_bar.clone(), cx);
            cx.refresh_windows();
        }
    });

    cx.observe_global::<Theme>({
        let title = title.clone();
        let app_menu_bar = app_menu_bar.clone();
        move |cx| {
            update_app_menu(title.clone(), app_menu_bar.clone(), cx);
        }
    })
    .detach();

    app_menu_bar
}

fn update_app_menu(title: impl Into<SharedString>, app_menu_bar: Entity<AppMenuBar>, cx: &mut App) {
    let title: SharedString = title.into();

    cx.set_menus(build_menus(title.clone(), cx));
    let menus = build_menus(title, cx)
        .into_iter()
        .map(|menu| menu.owned())
        .collect();
    GlobalState::global_mut(cx).set_app_menus(menus);

    app_menu_bar.update(cx, |menu_bar, cx| {
        menu_bar.reload(cx);
    })
}

fn build_menus(title: impl Into<SharedString>, cx: &App) -> Vec<Menu> {
    vec![
        Menu {
            name: title.into(),
            items: vec![
                MenuItem::action(t!("menu.about"), About),
                MenuItem::Separator,
                MenuItem::Submenu(Menu {
                    name: t!("menu.appearance").into(),
                    items: vec![
                        MenuItem::action(t!("menu.light"), SwitchThemeMode(ThemeMode::Light))
                            .checked(!cx.theme().mode.is_dark()),
                        MenuItem::action(t!("menu.dark"), SwitchThemeMode(ThemeMode::Dark))
                            .checked(cx.theme().mode.is_dark()),
                    ],
                    disabled: false,
                }),
                theme_menu(cx),
                language_menu(),
                MenuItem::Separator,
                MenuItem::action(t!("menu.quit"), Quit),
            ],
            disabled: false,
        },
        Menu {
            name: t!("menu.edit").into(),
            items: vec![
                MenuItem::action(t!("menu.undo"), gpui_component::input::Undo),
                MenuItem::action(t!("menu.redo"), gpui_component::input::Redo),
                MenuItem::separator(),
                MenuItem::action(t!("menu.cut"), gpui_component::input::Cut),
                MenuItem::action(t!("menu.copy"), gpui_component::input::Copy),
                MenuItem::action(t!("menu.paste"), gpui_component::input::Paste),
                MenuItem::separator(),
                MenuItem::action(t!("menu.select_all"), gpui_component::input::SelectAll),
            ],
            disabled: false,
        },
    ]
}

fn language_menu() -> MenuItem {
    let current = i18n::locale().to_string();
    MenuItem::Submenu(Menu {
        name: t!("menu.language").into(),
        items: vec![
            MenuItem::action("English", SelectLocale("en".into())).checked(current == "en"),
            MenuItem::action("简体中文", SelectLocale("zh-CN".into()))
                .checked(current == "zh-CN"),
        ],
        disabled: false,
    })
}

fn theme_menu(cx: &App) -> MenuItem {
    let themes = ThemeRegistry::global(cx).sorted_themes();
    let current_name = cx.theme().theme_name();
    MenuItem::Submenu(Menu {
        name: t!("menu.theme").into(),
        items: themes
            .iter()
            .map(|theme| {
                let checked = current_name == &theme.name;
                MenuItem::action(theme.name.clone(), SwitchTheme(theme.name.clone()))
                    .checked(checked)
            })
            .collect(),
        disabled: false,
    })
}
