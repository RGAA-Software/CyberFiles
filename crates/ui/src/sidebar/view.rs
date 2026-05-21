use std::path::Path;

use cyberfiles_core::{load_config, sidebar_is_compact, sidebar_is_offcanvas};
use cyberfiles_platform_windows::{icon_hint_for_path, open_item_properties, ShellIconHint};
use gpui::{prelude::*, *};
use gpui_component::{
    menu::{PopupMenu, PopupMenuItem},
    sidebar::{
        Sidebar, SidebarCollapsible, SidebarGroup, SidebarHeader, SidebarItem, SidebarMenu,
        SidebarMenuItem, SidebarToggleButton,
    },
    h_flex, v_flex, ActiveTheme as _, Icon, IconName,
};
use rust_i18n::t;

use crate::main_page::MainPage;
use crate::shell::navigation::NavigationTarget;

use gpui_component::sidebar::FilePathDrag;

use super::model::{SidebarEntry, SidebarSection};

pub fn render_sidebar(
    page: Entity<MainPage>,
    active: NavigationTarget,
    sections: &[SidebarSection],
    window: &mut Window,
    cx: &mut Context<MainPage>,
) -> impl IntoElement {
    let config = load_config().unwrap_or_default();
    let collapsed = config.sidebar_collapsed;
    let collapsible = if sidebar_is_offcanvas(&config) {
        SidebarCollapsible::Offcanvas
    } else if sidebar_is_compact(&config) {
        SidebarCollapsible::Icon
    } else {
        SidebarCollapsible::None
    };

    let settings_entry = SidebarEntry {
        id: "settings".into(),
        label: t!("nav.settings").to_string(),
        target: NavigationTarget::Settings,
        pinned_in_settings: false,
    };

    let mut footer = h_flex()
        .w_full()
        .gap_2()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    SidebarMenu::new()
                        .w_full()
                        .child(menu_item_for_entry(
                            &page,
                            &settings_entry,
                            &active,
                            collapsed,
                        ))
                        .render("sidebar-settings", window, cx),
                ),
        );
    if collapsible != SidebarCollapsible::None {
        footer = footer.child(
            SidebarToggleButton::new()
                .collapsed(collapsed)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_sidebar_collapsed(cx);
                })),
        );
    }

    let mut sidebar = Sidebar::new("files-sidebar")
        .collapsible(collapsible)
        .collapsed(collapsed)
        .w_full()
        .min_w_0()
        .border_0()
        .header(render_sidebar_header(cx))
        .footer(footer);

    for section in sections {
        let menu = SidebarMenu::new().children(section.entries.iter().map(|entry| {
            menu_item_for_entry(&page, entry, &active, collapsed)
        }));
        sidebar = sidebar.child(SidebarGroup::new(section.title.clone()).child(menu));
    }

    sidebar
}

fn render_sidebar_header(cx: &App) -> SidebarHeader {
    SidebarHeader::new().child(
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .rounded(cx.theme().radius_lg)
                    .bg(cx.theme().primary)
                    .text_color(cx.theme().primary_foreground)
                    .size_8()
                    .flex_shrink_0()
                    .child(Icon::new(IconName::GalleryVerticalEnd)),
            )
            .child(
                v_flex()
                    .gap_0()
                    .text_sm()
                    .child(cyberfiles_core::APP_NAME)
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t!("sidebar.workspace")),
                    ),
            ),
    )
}

fn menu_item_for_entry(
    page: &Entity<MainPage>,
    entry: &SidebarEntry,
    active: &NavigationTarget,
    collapsed: bool,
) -> SidebarMenuItem {
    let target = entry.target.clone();
    let is_active = navigation_matches(active, &target);
    let icon = icon_for_target(&target);
    let page_click = page.clone();
    let page_middle = page.clone();
    let page_menu = page.clone();
    let entry = entry.clone();

    let mut item = SidebarMenuItem::new(entry.label.clone())
        .icon(icon)
        .active(is_active)
        .collapsed(collapsed)
        .on_click({
            let target = target.clone();
            move |_, _, cx| {
                let _ = page_click.update(cx, |page, cx| {
                    page.navigate_to(target.clone(), cx);
                });
            }
        })
        .on_middle_click({
            let target = target.clone();
            move |_, cx| {
                if let NavigationTarget::Path(path) = &target {
                    let _ = page_middle.update(cx, |page, cx| {
                        page.open_path_in_new_tab(path.clone(), cx);
                    });
                }
            }
        })
        .context_menu(move |menu, window, cx| {
            build_entry_context_menu(menu, &page_menu, &entry, window, cx)
        });

    if let Some(dest) = drop_destination(&target) {
        let page_drop = page.clone();
        let page_hover = page.clone();
        let dest_hover = dest.clone();
        let dest_drop = dest.clone();
        item = item
            .on_file_drag_move(move |_, cx| {
                let path = dest_hover.clone();
                let _ = page_hover.update(cx, |page, cx| {
                    page.schedule_breadcrumb_drag_preview(path, cx);
                });
            })
            .on_file_drop(move |paths: &FilePathDrag, window, cx| {
                let path = dest_drop.clone();
                let _ = page_drop.update(cx, |page, cx| {
                    page.drop_paths_on_directory(path, paths.0.clone(), window, cx);
                });
            });
    }

    item
}

fn drop_destination(target: &NavigationTarget) -> Option<std::path::PathBuf> {
    match target {
        NavigationTarget::Path(path) if path.is_dir() => Some(path.clone()),
        _ => None,
    }
}

fn build_entry_context_menu(
    menu: PopupMenu,
    page: &Entity<MainPage>,
    entry: &SidebarEntry,
    _window: &mut Window,
    cx: &mut App,
) -> PopupMenu {
    let target = entry.target.clone();
    let pinned = entry.pinned_in_settings;

    let page_nav = page.clone();
    let nav_target = target.clone();
    let mut menu = menu.item(
        PopupMenuItem::new(t!("sidebar.menu.open"))
            .on_click(move |_, _, cx| {
                let _ = page_nav.update(cx, |p, cx| p.navigate_to(nav_target.clone(), cx));
            }),
    );

    if let NavigationTarget::Path(path) = target.clone() {
        let path_exists = path.exists();
        let path_string = path.to_string_lossy().to_string();

        let page_tab = page.clone();
        let path_tab = path.clone();
        menu = menu.item(
            PopupMenuItem::new(t!("sidebar.menu.open_new_tab")).on_click(move |_, _, cx| {
                let _ = page_tab.update(cx, |p, cx| p.open_path_in_new_tab(path_tab.clone(), cx));
            }),
        );

        if pinned {
            let page_unpin = page.clone();
            let ps_unpin = path_string.clone();
            menu = menu.item(
                PopupMenuItem::new(t!("sidebar.menu.unpin")).on_click(move |_, _, cx| {
                    let _ = page_unpin.update(cx, |p, cx| {
                        p.unpin_folder_path(&ps_unpin, cx);
                    });
                }),
            );
            let page_up = page.clone();
            let ps_up = path_string.clone();
            menu = menu.item(
                PopupMenuItem::new(t!("sidebar.menu.move_up")).on_click(move |_, _, cx| {
                    let _ = page_up.update(cx, |p, cx| p.move_pinned_folder(&ps_up, -1, cx));
                }),
            );
            let page_down = page.clone();
            let ps_down = path_string.clone();
            menu = menu.item(
                PopupMenuItem::new(t!("sidebar.menu.move_down")).on_click(move |_, _, cx| {
                    let _ = page_down.update(cx, |p, cx| p.move_pinned_folder(&ps_down, 1, cx));
                }),
            );
        } else if path_exists {
            let page_pin = page.clone();
            let path_pin = path.clone();
            menu = menu.item(PopupMenuItem::new(t!("sidebar.menu.pin")).on_click(
                move |_, _, cx| {
                    let _ = page_pin.update(cx, |p, cx| p.pin_folder_path(path_pin.clone(), cx));
                },
            ));
        }

        let path_props = path.clone();
        menu = menu.item(
            PopupMenuItem::new(t!("sidebar.menu.properties")).on_click(move |_, _, cx| {
                let _ = open_item_properties(&path_props);
                cx.stop_propagation();
            }),
        );
    }

    menu
}

fn icon_for_target(target: &NavigationTarget) -> Icon {
    match target {
        NavigationTarget::Home => Icon::new(IconName::LayoutDashboard),
        NavigationTarget::RecycleBin => Icon::new(IconName::Delete),
        NavigationTarget::Settings => Icon::new(IconName::Settings2),
        NavigationTarget::Path(path) => {
            let name = match icon_hint_for_path(path) {
                ShellIconHint::Folder => IconName::Folder,
                ShellIconHint::File => IconName::File,
                ShellIconHint::Symlink => IconName::ExternalLink,
                ShellIconHint::Executable => IconName::File,
                ShellIconHint::Image => IconName::File,
                ShellIconHint::Archive => IconName::Folder,
            };
            Icon::new(name)
        }
    }
}

pub fn navigation_matches(active: &NavigationTarget, entry: &NavigationTarget) -> bool {
    match (active, entry) {
        (NavigationTarget::Home, NavigationTarget::Home) => true,
        (NavigationTarget::RecycleBin, NavigationTarget::RecycleBin) => true,
        (NavigationTarget::Settings, NavigationTarget::Settings) => true,
        (NavigationTarget::Path(current), NavigationTarget::Path(sidebar)) => {
            paths_match(sidebar, current)
        }
        _ => false,
    }
}

fn paths_match(sidebar: &Path, current: &Path) -> bool {
    if paths_equal(sidebar, current) {
        return true;
    }
    // Drive roots (C:\) highlight only when browsing that root, not the whole tree.
    if is_windows_drive_root(sidebar) {
        return false;
    }
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(sidebar), std::fs::canonicalize(current)) {
        return is_strict_descendant(&a, &b);
    }
    is_strict_descendant(sidebar, current)
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        return a == b;
    }
    false
}

/// True when `path` is a strict child of `ancestor` (not equal).
fn is_strict_descendant(ancestor: &Path, path: &Path) -> bool {
    let ancestor_components: Vec<_> = ancestor.components().collect();
    let mut path_components: Vec<_> = path.components().collect();
    if path_components.len() <= ancestor_components.len() {
        return false;
    }
    path_components.truncate(ancestor_components.len());
    path_components == ancestor_components
}

#[cfg(windows)]
fn is_windows_drive_root(path: &Path) -> bool {
    use std::path::Component;
    let components: Vec<_> = path.components().collect();
    match components.as_slice() {
        [Component::Prefix(prefix)] => {
            let s = prefix.as_os_str().to_string_lossy();
            s.len() == 2 && s.ends_with(':')
        }
        [Component::Prefix(prefix), Component::RootDir] => {
            let s = prefix.as_os_str().to_string_lossy();
            s.len() == 2 && s.ends_with(':')
        }
        _ => false,
    }
}

#[cfg(not(windows))]
fn is_windows_drive_root(_path: &Path) -> bool {
    false
}
