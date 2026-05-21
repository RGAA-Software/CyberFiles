use std::path::{Path, PathBuf};

use cyberfiles_core::{load_config, sidebar_is_compact, sidebar_is_offcanvas};
use cyberfiles_platform_windows::{open_item_properties, SHELL_RECYCLE_BIN_PATH};
use gpui::{prelude::*, ClickEvent, *};
use gpui_component::{
    menu::{PopupMenu, PopupMenuItem},
    sidebar::{
        Sidebar, SidebarCollapsible, SidebarGroup, SidebarItem, SidebarMenu, SidebarMenuItem,
        SidebarToggleButton,
    },
    Collapsible,
    h_flex, ActiveTheme as _, Icon, IconName,
};
use rust_i18n::t;

use crate::drag::DraggedFilePaths;
use crate::main_page::MainPage;
use crate::shell::navigation::NavigationTarget;

use super::menu_with_drop::SidebarMenuWithDrop;
use super::model::{SidebarEntry, SidebarSection, SidebarSectionKind};

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
                        .child(menu_item_for_entry(&page, &settings_entry, &active, collapsed).0)
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
        .footer(footer);

    for section in sections {
        let mut menu = SidebarMenuWithDrop::new();
        for entry in &section.entries {
            append_sidebar_entry(&mut menu, &page, entry, &active, collapsed);
        }
        let block = if section.kind == SidebarSectionKind::Home {
            SidebarSectionBlock::flat(menu)
        } else {
            SidebarSectionBlock::group(SidebarGroup::new(section.title.clone()).child(menu))
        };
        sidebar = sidebar.child(block);
    }

    sidebar
}

/// Top sidebar entries (home, recycle bin) without a section heading.
#[derive(Clone)]
enum SidebarSectionBlock {
    Flat(SidebarMenuWithDrop),
    Group(SidebarGroup<SidebarMenuWithDrop>),
}

impl SidebarSectionBlock {
    fn flat(menu: SidebarMenuWithDrop) -> Self {
        Self::Flat(menu)
    }

    fn group(group: SidebarGroup<SidebarMenuWithDrop>) -> Self {
        Self::Group(group)
    }
}

impl gpui_component::Collapsible for SidebarSectionBlock {
    fn is_collapsed(&self) -> bool {
        match self {
            Self::Flat(menu) => menu.is_collapsed(),
            Self::Group(group) => group.is_collapsed(),
        }
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        match self {
            Self::Flat(menu) => Self::Flat(menu.collapsed(collapsed)),
            Self::Group(group) => Self::Group(group.collapsed(collapsed)),
        }
    }
}

impl SidebarItem for SidebarSectionBlock {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        match self {
            Self::Flat(menu) => menu.render(id, window, cx).into_any_element(),
            Self::Group(group) => group.render(id, window, cx).into_any_element(),
        }
    }
}

fn append_sidebar_entry(
    menu: &mut SidebarMenuWithDrop,
    page: &Entity<MainPage>,
    entry: &SidebarEntry,
    active: &NavigationTarget,
    collapsed: bool,
) {
    if let Some(shell_path) = shell_path_for_target(&entry.target) {
        push_shell_sidebar_entry(menu, page, entry, active, collapsed, shell_path);
        return;
    }

    let (item, middle_click) = menu_item_for_entry(page, entry, active, collapsed);
    menu.push_child(item, middle_click);
}

fn shell_path_for_target(target: &NavigationTarget) -> Option<std::path::PathBuf> {
    match target {
        NavigationTarget::Path(path) => Some(path.clone()),
        NavigationTarget::Home => std::env::var_os("USERPROFILE").map(std::path::PathBuf::from),
        // Files uses `Shell:RecycleBinFolder` for the colorful recycle bin icon, not the FS path.
        NavigationTarget::RecycleBin => Some(PathBuf::from(SHELL_RECYCLE_BIN_PATH)),
        _ => None,
    }
}

fn push_shell_sidebar_entry(
    menu: &mut SidebarMenuWithDrop,
    page: &Entity<MainPage>,
    entry: &SidebarEntry,
    active: &NavigationTarget,
    collapsed: bool,
    shell_path: std::path::PathBuf,
) {
    let is_active = navigation_matches(active, &entry.target);
    let page_click = page.clone();
    let page_middle = page.clone();
    let page_menu = page.clone();
    let entry = entry.clone();
    let target = entry.target.clone();
    let label = SharedString::from(entry.label.clone());

    let target_click = target.clone();
    let handler = move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        let _ = page_click.update(cx, |page, cx| {
            page.navigate_to(target_click.clone(), cx);
        });
    };

    let middle_click: Option<std::rc::Rc<dyn Fn(&mut Window, &mut App)>> =
        if matches!(&target, NavigationTarget::Path(_)) {
            let target = target.clone();
            Some(std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
                if let NavigationTarget::Path(path) = &target {
                    let _ = page_middle.update(cx, |page, cx| {
                        page.open_path_in_new_tab(path.clone(), cx);
                    });
                }
            }))
        } else {
            None
        };

    let entry_menu = entry.clone();
    let context_menu: Option<std::rc::Rc<dyn Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu>> =
        Some(std::rc::Rc::new(move |menu, window, cx| {
            build_entry_context_menu(menu, &page_menu, &entry_menu, window, cx)
        }));

    let drop_dest = drop_destination(&entry.target);
    if let Some(dest) = drop_dest {
        let page_drop = page.clone();
        let page_hover = page.clone();
        let dest_hover = dest.clone();
        let dest_drop = dest.clone();
        menu.push_shell_path_with_folder_drop(
            label,
            shell_path,
            is_active,
            collapsed,
            handler,
            middle_click,
            context_menu,
            move |_, cx| {
                let path = dest_hover.clone();
                let _ = page_hover.update(cx, |page, cx| {
                    page.schedule_breadcrumb_drag_preview(path, cx);
                });
            },
            move |paths: &DraggedFilePaths, window, cx| {
                let path = dest_drop.clone();
                let _ = page_drop.update(cx, |page, cx| {
                    page.drop_paths_on_directory(path, paths.0.clone(), window, cx);
                });
            },
        );
    } else {
        menu.push_shell_path(
            label,
            shell_path,
            is_active,
            collapsed,
            handler,
            middle_click,
            context_menu,
        );
    }
}

fn menu_item_for_entry(
    page: &Entity<MainPage>,
    entry: &SidebarEntry,
    active: &NavigationTarget,
    collapsed: bool,
) -> (SidebarMenuItem, Option<std::rc::Rc<dyn Fn(&mut Window, &mut App)>>) {
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
        .context_menu(move |menu, window, cx| {
            build_entry_context_menu(menu, &page_menu, &entry, window, cx)
        });

    let middle_click: Option<std::rc::Rc<dyn Fn(&mut Window, &mut App)>> =
        if matches!(&target, NavigationTarget::Path(_)) {
            let target = target.clone();
            Some(std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
                if let NavigationTarget::Path(path) = &target {
                    let _ = page_middle.update(cx, |page, cx| {
                        page.open_path_in_new_tab(path.clone(), cx);
                    });
                }
            }))
        } else {
            None
        };

    (item, middle_click)
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
        NavigationTarget::FileTag(_) => Icon::new(IconName::Inbox),
        NavigationTarget::Path(_) => Icon::new(IconName::Folder),
    }
}

pub fn navigation_matches(active: &NavigationTarget, entry: &NavigationTarget) -> bool {
    match (active, entry) {
        (NavigationTarget::Home, NavigationTarget::Home) => true,
        (NavigationTarget::RecycleBin, NavigationTarget::RecycleBin) => true,
        (NavigationTarget::Settings, NavigationTarget::Settings) => true,
        (NavigationTarget::FileTag(active), NavigationTarget::FileTag(entry)) => active == entry,
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
