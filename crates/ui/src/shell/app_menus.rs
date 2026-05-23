use gpui::{App, Entity, Global, Menu, MenuItem, SharedString};
use gpui_component::{GlobalState, menu::AppMenuBar};

use rust_i18n::t;

use cyberfiles_commands::ReopenClosedTab;

use super::actions::{About, Quit};

struct AppMenuState {
    menu_bar: Entity<AppMenuBar>,
    title: SharedString,
}

impl Global for AppMenuState {}

pub fn menu_bar(cx: &App) -> Entity<AppMenuBar> {
    cx.global::<AppMenuState>().menu_bar.clone()
}

pub fn init(title: impl Into<SharedString>, cx: &mut App) -> Entity<AppMenuBar> {
    let app_menu_bar = AppMenuBar::new(cx);
    let title: SharedString = title.into();
    cx.set_global(AppMenuState {
        menu_bar: app_menu_bar.clone(),
        title: title.clone(),
    });
    update_app_menu(cx);

    app_menu_bar
}

/// Reload native and in-window menus (e.g. after locale change).
pub fn reload(cx: &mut App) {
    if !cx.has_global::<AppMenuState>() {
        return;
    }
    update_app_menu(cx);
}

fn update_app_menu(cx: &mut App) {
    let state = cx.global::<AppMenuState>();
    let title = state.title.clone();
    let app_menu_bar = state.menu_bar.clone();

    cx.set_menus(build_menus(title.clone()));
    let menus = build_menus(title)
        .into_iter()
        .map(|menu| menu.owned())
        .collect();
    GlobalState::global_mut(cx).set_app_menus(menus);

    app_menu_bar.update(cx, |menu_bar, cx| {
        menu_bar.reload(cx);
    });
}

fn build_menus(title: impl Into<SharedString>) -> Vec<Menu> {
    vec![
        Menu {
            name: title.into(),
            items: vec![
                MenuItem::action(t!("menu.about"), About),
                MenuItem::Separator,
                MenuItem::action(t!("menu.quit"), Quit),
            ],
            disabled: false,
        },
        Menu {
            name: t!("menu.view").into(),
            items: vec![MenuItem::action(
                t!("nav.reopen_closed_tab"),
                ReopenClosedTab,
            )],
            disabled: false,
        },
    ]
}
