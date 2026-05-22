//! Files-style content page context flyout (`ContentPageContextFlyoutFactory` layout).

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use cyberfiles_commands::{
    CopyItems, CopyPath, CutItems, DeleteItems, DeleteItemsPermanent, NewFile, NewFolder, OpenItem,
    PasteItems, RefreshDirectory, RenameItem, ShellProperties, ViewColumns, ViewDetails, ViewGrid,
};
use cyberfiles_core::load_config;
use cyberfiles_fs::SortOption;
use cyberfiles_platform_windows::{self as platform, ShellContextMenuEntry};
use gpui::{div, prelude::*, Context, Entity, Pixels, SharedString, Window, px};
use gpui_component::{
    h_flex,
    menu::{PopupMenu, PopupMenuItem},
    notification::Notification,
    Disableable as _, Icon, IconName, Size, Sizable as _, WindowExt as _,
};
use rust_i18n::t;

use super::{
    BrowseLocation, CreateFolderFromSelection, CreateShortcut, FileBrowser, OpenInNewPane,
    OpenInNewWindow, OpenInTerminal, OpenWithDialog, ShellSubmenuSnapshot, SortByCreated,
    SortByModified, SortByName, SortBySize, SortByType, ToggleShowHidden, ToggleSortDirection,
    ViewMode, shell_submenu_snapshot,
};
use crate::app_state::{AppFileClipboard, AppNavigation};
use crate::icons::toolbar_icon;
use crate::shell_menu_icon::shell_menu_icon_img;
use crate::toolbar_button::toolbar_icon_button;

/// Max height for Shell extension lists (`PopupMenu::scrollable` + nested branch flyouts).
const SHELL_MENU_MAX_HEIGHT: Pixels = px(620.);

/// Matches gpui-component `PopupMenu` default row height (26px).
const CONTEXT_MENU_ROW_HEIGHT: Pixels = px(26.);
const CONTEXT_MENU_ICON_SIZE: Pixels = px(16.);
/// Matches gpui-component popup menu `INNER_PADDING`.
const CONTEXT_MENU_INNER_PADDING: Pixels = px(8.);

fn apply_context_menu_style(menu: PopupMenu) -> PopupMenu {
    menu
}

fn context_menu_icon_cell(icon: Icon) -> impl IntoElement {
    div()
        .w(CONTEXT_MENU_ICON_SIZE)
        .h(CONTEXT_MENU_ICON_SIZE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(icon.xsmall())
}

fn context_menu_icon_cell_empty() -> impl IntoElement {
    div()
        .w(CONTEXT_MENU_ICON_SIZE)
        .h(CONTEXT_MENU_ICON_SIZE)
        .flex_none()
}

fn context_menu_shell_icon_cell(png: Arc<Vec<u8>>, window: &Window) -> impl IntoElement {
    div()
        .w(CONTEXT_MENU_ICON_SIZE)
        .h(CONTEXT_MENU_ICON_SIZE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .child(shell_menu_icon_img(png, window))
}

fn context_menu_text_row(
    label: SharedString,
    icon: Option<Icon>,
    trailing_chevron: bool,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .h(CONTEXT_MENU_ROW_HEIGHT)
        .min_h(CONTEXT_MENU_ROW_HEIGHT)
        .items_center()
        .gap_2()
        .px(CONTEXT_MENU_INNER_PADDING)
        .child(if let Some(icon) = icon {
            context_menu_icon_cell(icon).into_any_element()
        } else {
            context_menu_icon_cell_empty().into_any_element()
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .child(label),
        )
        .when(trailing_chevron, |row| {
            row.child(Icon::new(IconName::ChevronRight).xsmall())
        })
}

fn context_menu_action_item(
    label: impl Into<SharedString>,
    icon: Icon,
    action: Box<dyn gpui::Action>,
    disabled: bool,
    checked: bool,
) -> PopupMenuItem {
    let label = label.into();
    let left_icon = if checked {
        Icon::new(IconName::Check)
    } else {
        icon.clone()
    };
    PopupMenuItem::element(move |_, _| {
        context_menu_text_row(label.clone(), Some(left_icon.clone()), false)
    })
        .action(action)
        .disabled(disabled)
}

fn finish_shell_popup_menu(menu: PopupMenu) -> PopupMenu {
    menu.scrollable(true).max_h(SHELL_MENU_MAX_HEIGHT)
}

fn entries_contain_submenu(entries: &[ShellContextMenuEntry]) -> bool {
    entries
        .iter()
        .any(|entry| matches!(entry, ShellContextMenuEntry::Submenu { .. }))
}

fn resolve_submenu_entries(
    lazy_parent_index: Option<u32>,
    children: &[ShellContextMenuEntry],
) -> Vec<ShellContextMenuEntry> {
    if let Some(index) = lazy_parent_index {
        match std::thread::spawn(move || platform::load_lazy_submenu(index)).join() {
            Ok(Ok(items)) => items,
            Ok(Err(error)) => {
                eprintln!("[shell-menu] lazy submenu err: {error:#}");
                Vec::new()
            }
            Err(_) => Vec::new(),
        }
    } else {
        children.to_vec()
    }
}

/// Build the file list context menu (background or item flyout).
pub fn build_context_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let menu = if browser.read(cx).is_background_context_menu() {
        build_background_menu(menu, browser, window, cx)
    } else {
        build_item_menu(menu, browser, window, cx)
    };
    apply_context_menu_style(menu)
}

fn build_background_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let can_paste = AppFileClipboard::has_items(cx);
    let state = browser.read(cx);
    let focus = state.focus_handle.clone();
    let in_recycle = state.browse_location == BrowseLocation::RecycleBin;
    let in_file_tag = matches!(state.browse_location, BrowseLocation::FileTag { .. });

    let browser_layout = browser.clone();
    let browser_sort = browser.clone();
    let browser_new = browser.clone();

    let mut menu = menu.action_context(focus);

    if !in_recycle && !in_file_tag {
        menu = menu
            .menu_with_enable(t!("files.menu.paste"), Box::new(PasteItems), can_paste)
            .separator();
    }

    menu = menu.submenu(t!("files.menu.layout"), window, cx, move |menu, _, cx| {
        let focus = browser_layout.read(cx).focus_handle.clone();
        let view_mode = browser_layout.read(cx).view_mode;
        menu.action_context(focus)
            .item(context_menu_action_item(
                t!("files.view.details"),
                Icon::new(IconName::GalleryVerticalEnd),
                Box::new(ViewDetails),
                false,
                view_mode == ViewMode::Details,
            ))
            .item(context_menu_action_item(
                t!("files.view.grid"),
                Icon::new(IconName::LayoutDashboard),
                Box::new(ViewGrid),
                false,
                view_mode == ViewMode::Grid,
            ))
            .item(context_menu_action_item(
                t!("files.view.columns"),
                Icon::new(IconName::PanelLeft),
                Box::new(ViewColumns),
                false,
                view_mode == ViewMode::Columns,
            ))
    })
    .submenu(t!("files.menu.sort"), window, cx, move |menu, _, cx| {
        let state = browser_sort.read(cx);
        let focus = state.focus_handle.clone();
        let sort = state.sort_preferences;
        let show_hidden = state.read_options.show_hidden_items;
        let hidden_label = if show_hidden {
            t!("files.show_hidden.off")
        } else {
            t!("files.show_hidden.on")
        };
        menu.action_context(focus)
            .item(context_menu_action_item(
                t!("files.sort.name"),
                Icon::empty(),
                Box::new(SortByName),
                false,
                sort.option == SortOption::Name,
            ))
            .item(context_menu_action_item(
                t!("files.sort.modified"),
                Icon::empty(),
                Box::new(SortByModified),
                false,
                sort.option == SortOption::DateModified,
            ))
            .item(context_menu_action_item(
                t!("files.sort.created"),
                Icon::empty(),
                Box::new(SortByCreated),
                false,
                sort.option == SortOption::DateCreated,
            ))
            .item(context_menu_action_item(
                t!("files.sort.size"),
                Icon::empty(),
                Box::new(SortBySize),
                false,
                sort.option == SortOption::Size,
            ))
            .item(context_menu_action_item(
                t!("files.sort.type"),
                Icon::empty(),
                Box::new(SortByType),
                false,
                sort.option == SortOption::FileType,
            ))
            .separator()
            .item(context_menu_action_item(
                t!("files.sort.toggle_direction"),
                Icon::empty(),
                Box::new(ToggleSortDirection),
                false,
                false,
            ))
            .item(context_menu_action_item(
                hidden_label,
                Icon::empty(),
                Box::new(ToggleShowHidden),
                false,
                false,
            ))
    });

    if !in_recycle && !in_file_tag {
        menu = menu
            .separator()
            .submenu(t!("files.menu.new"), window, cx, move |menu, _, cx| {
                let focus = browser_new.read(cx).focus_handle.clone();
                menu.action_context(focus)
                    .menu(t!("files.new_folder"), Box::new(NewFolder))
                    .menu(t!("files.new_file"), Box::new(NewFile))
            });
    }

    menu.menu(t!("files.menu.refresh"), Box::new(RefreshDirectory))
}

fn build_item_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    match browser.read(cx).browse_location.clone() {
        BrowseLocation::RecycleBin => build_recycle_item_menu(menu, browser, window, cx),
        BrowseLocation::FileTag { .. } => build_file_tag_item_menu(menu, browser, window, cx),
        BrowseLocation::Directory => build_directory_item_menu(menu, browser, window, cx),
    }
}

/// Files item flyout: icon toolbar → open group → path/organize → terminal → show more (Shell).
fn build_directory_item_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let can_paste = AppFileClipboard::has_items(cx);
    let state = browser.read(cx);
    let paths = state.selected_paths_vec();
    let has_selection = !paths.is_empty();
    let single = paths.len() == 1;
    let single_dir = single && paths[0].is_dir();
    let multi = paths.len() > 1;
    let focus = state.focus_handle.clone();
    let extended = state.context_menu_extended_verbs;

    let mut menu = menu.action_context(focus);

    menu = append_quick_action_toolbar(menu, has_selection, can_paste);
    menu = menu.separator();

    menu = menu.item(context_menu_action_item(
        t!("files.menu.open"),
        Icon::new(IconName::Folder),
        Box::new(OpenItem),
        false,
        false,
    ));

    if single && !paths[0].is_dir() {
        menu = menu.item(context_menu_action_item(
            t!("files.menu.open_with"),
            Icon::new(IconName::Settings2),
            Box::new(OpenWithDialog),
            false,
            false,
        ));
    }

    if single {
        let path = paths[0].clone();
        let tab_label: SharedString = t!("sidebar.menu.open_new_tab").into();
        let tab_path = path.clone();
        menu = menu.item(
            PopupMenuItem::element(move |_, _| {
                context_menu_text_row(tab_label.clone(), Some(Icon::new(IconName::File)), false)
            })
            .on_click(move |_, _, cx| {
                AppNavigation::open_path_in_new_tab(tab_path.clone(), cx);
            }),
        );
        menu = menu
            .item(context_menu_action_item(
                t!("files.menu.open_in_new_window"),
                Icon::new(IconName::ExternalLink),
                Box::new(OpenInNewWindow),
                false,
                false,
            ))
            .item(context_menu_action_item(
                t!("files.menu.open_in_new_pane"),
                Icon::new(IconName::PanelLeft),
                Box::new(OpenInNewPane),
                false,
                false,
            ));
    }

    menu = menu.separator();

    menu = menu.item(context_menu_action_item(
        t!("files.menu.copy_path"),
        Icon::new(IconName::ExternalLink),
        Box::new(CopyPath),
        false,
        false,
    ));

    if multi {
        menu = menu.item(context_menu_action_item(
            t!("files.menu.create_folder_from_selection"),
            Icon::new(IconName::Folder),
            Box::new(CreateFolderFromSelection),
            false,
            false,
        ));
    }

    if single && !paths[0].is_dir() {
        menu = menu.item(context_menu_action_item(
            t!("files.menu.create_shortcut"),
            Icon::new(IconName::ExternalLink),
            Box::new(CreateShortcut),
            false,
            false,
        ));
    }

    menu = menu
        .item(placeholder_item(
            t!("files.menu.compress"),
            IconName::Folder,
            t!("files.menu.not_implemented"),
        ))
        .item(placeholder_item(
            t!("files.menu.send_to"),
            IconName::ExternalLink,
            t!("files.menu.not_implemented"),
        ));

    if single_dir {
        let path = paths[0].clone();
        let path_string = path.to_string_lossy().to_string();
        let pin_label = if path_is_pinned(&path_string) {
            t!("sidebar.menu.unpin")
        } else {
            t!("sidebar.menu.pin")
        };
        let pin_icon = Icon::new(IconName::Folder);
        if path_is_pinned(&path_string) {
            let ps = path_string.clone();
            let label: SharedString = pin_label.into();
            menu = menu.item(
                PopupMenuItem::element(move |_, _| {
                    context_menu_text_row(label.clone(), Some(pin_icon.clone()), false)
                })
                .on_click(move |_, _, cx| AppNavigation::unpin_folder(&ps, cx)),
            );
        } else if path.exists() {
            let label: SharedString = pin_label.into();
            let pin_path = path.clone();
            menu = menu.item(
                PopupMenuItem::element(move |_, _| {
                    context_menu_text_row(label.clone(), Some(pin_icon.clone()), false)
                })
                .on_click(move |_, _, cx| AppNavigation::pin_folder(pin_path.clone(), cx)),
            );
        }
    }

    if single {
        if let Some(parent) = paths[0].parent() {
            let parent = parent.to_path_buf();
            let loc_label: SharedString = t!("files.menu.open_file_location").into();
            let loc_parent = parent.clone();
            menu = menu.item(
                PopupMenuItem::element(move |_, _| {
                    context_menu_text_row(
                        loc_label.clone(),
                        Some(Icon::new(IconName::Folder)),
                        false,
                    )
                })
                .on_click(move |_, _, cx| {
                    AppNavigation::navigate_to_path(loc_parent.clone(), cx);
                }),
            );
        }
    }

    menu = menu
        .separator()
        .item(context_menu_action_item(
            t!("files.menu.open_in_terminal"),
            Icon::new(IconName::File),
            Box::new(OpenInTerminal),
            false,
            false,
        ))
        .item(placeholder_item(
            t!("files.menu.edit_file_tags"),
            IconName::File,
            t!("files.menu.not_implemented"),
        ))
        .separator();

    if has_selection {
        menu = append_show_more_options(
            menu,
            paths,
            extended,
            state.shell_menu_cache.clone(),
            browser,
            window,
            cx,
        );
    }

    menu
}

fn append_quick_action_toolbar(
    menu: PopupMenu,
    has_selection: bool,
    can_paste: bool,
) -> PopupMenu {
    menu.item(
        PopupMenuItem::element(move |window, cx| {
            h_flex()
                .id("context-quick-actions")
                .h(CONTEXT_MENU_ROW_HEIGHT)
                .min_h(CONTEXT_MENU_ROW_HEIGHT)
                .items_center()
                .gap(px(2.))
                .px_2()
                .child(quick_toolbar_button(
                    "ctx-cut",
                    IconName::Replace,
                    Box::new(CutItems),
                    has_selection,
                    window,
                    cx,
                ))
                .child(quick_toolbar_button(
                    "ctx-copy",
                    IconName::Copy,
                    Box::new(CopyItems),
                    has_selection,
                    window,
                    cx,
                ))
                .child(quick_toolbar_button(
                    "ctx-paste",
                    IconName::Replace,
                    Box::new(PasteItems),
                    can_paste,
                    window,
                    cx,
                ))
                .child(quick_toolbar_button(
                    "ctx-rename",
                    IconName::File,
                    Box::new(RenameItem),
                    has_selection,
                    window,
                    cx,
                ))
                .child(quick_toolbar_button(
                    "ctx-delete",
                    IconName::Delete,
                    Box::new(DeleteItems),
                    has_selection,
                    window,
                    cx,
                ))
                .child(quick_toolbar_button(
                    "ctx-properties",
                    IconName::Settings2,
                    Box::new(ShellProperties),
                    has_selection,
                    window,
                    cx,
                ))
                .into_any_element()
        }),
    )
}

fn quick_toolbar_button(
    id: &'static str,
    icon: IconName,
    action: Box<dyn gpui::Action>,
    enabled: bool,
    window: &mut Window,
    cx: &mut gpui::App,
) -> impl gpui::IntoElement {
    toolbar_icon_button(id)
        .with_size(Size::Small)
        .icon(toolbar_icon(icon))
        .disabled(!enabled)
        .on_click(move |_, window, cx| {
            window.dispatch_action(action.boxed_clone(), cx);
        })
        .into_any_element()
}

fn append_show_more_options(
    menu: PopupMenu,
    paths: Vec<PathBuf>,
    extended_verbs: bool,
    shell_menu_cache: Arc<RwLock<Option<super::ShellMenuCache>>>,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let paths_for_sub = paths.clone();
    let menu_icon_pixel_size = platform::menu_icon_pixel_size(window.scale_factor());
    apply_context_menu_style(
        menu.submenu_with_icon(
            Some(Icon::new(IconName::Ellipsis)),
            t!("files.menu.show_more_options"),
            window,
            cx,
            move |sub, window, cx| {
            match shell_submenu_snapshot(
                &shell_menu_cache,
                &paths_for_sub,
                extended_verbs,
                menu_icon_pixel_size,
            ) {
            ShellSubmenuSnapshot::Loading => sub.item(
                PopupMenuItem::new(t!("files.menu.shell_loading")).disabled(true),
            ),
            ShellSubmenuSnapshot::Empty => sub.item(
                PopupMenuItem::new(t!("files.menu.shell_empty")).disabled(true),
            ),
            ShellSubmenuSnapshot::Ready(entries) => append_shell_entries(
                sub,
                &entries,
                &paths_for_sub,
                extended_verbs,
                browser.clone(),
                window,
                cx,
            ),
            }
            },
        ),
    )
}

fn placeholder_item(
    label: impl Into<SharedString>,
    icon: IconName,
    message: impl Into<SharedString>,
) -> PopupMenuItem {
    let message = message.into();
    let label = label.into();
    let menu_icon = Icon::new(icon);
    PopupMenuItem::element(move |_, _| {
        context_menu_text_row(label.clone(), Some(menu_icon.clone()), false)
    })
    .on_click(move |_, window, cx| {
        window.push_notification(Notification::info(message.clone()), cx);
    })
}

fn shell_menu_item_is_properties(command_string: Option<&str>, label: &str) -> bool {
    if command_string.is_some_and(|v| v.eq_ignore_ascii_case("properties")) {
        return true;
    }
    let lower = label.to_ascii_lowercase();
    lower.contains("properties") || lower.contains("属性")
}

fn shell_popup_item(
    label: String,
    icon_png: Option<Vec<u8>>,
    paths: Vec<PathBuf>,
    command_offset: u32,
    command_string: Option<String>,
    extended_verbs: bool,
) -> PopupMenuItem {
    let display_label = platform::format_shell_menu_label(&label);
    let is_properties = shell_menu_item_is_properties(command_string.as_deref(), &label);
    let invoke = move |_window: &mut Window, _cx: &mut gpui::App| {
        let result = if is_properties {
            platform::invoke_shell_properties(&paths)
        } else {
            platform::invoke_shell_context_menu_item(&paths, command_offset, extended_verbs)
        };
        if let Err(error) = result {
            eprintln!("[shell-menu] menu invoke failed: {error:#}");
        }
    };

    let row_label: SharedString = display_label.into();
    let row_png = icon_png.map(Arc::new);
    PopupMenuItem::element(move |window, _| {
        let icon_slot = if let Some(png) = row_png.clone() {
            context_menu_shell_icon_cell(png, window).into_any_element()
        } else {
            context_menu_icon_cell_empty().into_any_element()
        };
        h_flex()
            .w_full()
            .h(CONTEXT_MENU_ROW_HEIGHT)
            .min_h(CONTEXT_MENU_ROW_HEIGHT)
            .items_center()
            .gap_2()
            .px(CONTEXT_MENU_INNER_PADDING)
            .min_w(px(200.))
            .child(icon_slot)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .child(row_label.clone()),
            )
            .into_any_element()
    })
    .on_click(move |_, window, cx| invoke(window, cx))
}

fn append_shell_submenu_item(
    menu: PopupMenu,
    label: String,
    children: &[ShellContextMenuEntry],
    lazy_parent_index: Option<u32>,
    paths: &[PathBuf],
    extended_verbs: bool,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let display_label = platform::format_shell_menu_label(&label);
    let log_label = display_label.clone();
    let paths_for_sub = paths.to_vec();
    let browser_sub = browser.clone();
    let lazy_index = lazy_parent_index;
    let children_stash = children.to_vec();
    menu.submenu_with_icon(
        Some(Icon::empty()),
        display_label,
        window,
        cx,
        move |sub, window, cx| {
        let loaded = resolve_submenu_entries(lazy_index, &children_stash);
        eprintln!(
            "[shell-menu] submenu {:?} lazy={lazy_index:?} entries={}",
            log_label, loaded.len()
        );
        if loaded.is_empty() {
            sub.item(PopupMenuItem::new(t!("files.menu.shell_empty")).disabled(true))
        } else {
            append_shell_entries(
                sub,
                &loaded,
                &paths_for_sub,
                extended_verbs,
                browser_sub.clone(),
                window,
                cx,
            )
        }
        },
    )
}

fn append_shell_flat_popup_items(
    mut menu: PopupMenu,
    flat_entries: &[ShellContextMenuEntry],
    paths: &[PathBuf],
    extended_verbs: bool,
) -> PopupMenu {
    for entry in flat_entries {
        match entry {
            ShellContextMenuEntry::Separator => {
                menu = menu.separator();
            }
            ShellContextMenuEntry::Item {
                label,
                command_offset,
                command_string,
                icon_png,
                ..
            } => {
                menu = menu.item(shell_popup_item(
                    label.clone(),
                    icon_png.clone(),
                    paths.to_vec(),
                    *command_offset,
                    command_string.clone(),
                    extended_verbs,
                ));
            }
            ShellContextMenuEntry::Submenu { .. } => {}
        }
    }
    menu
}

pub(crate) fn append_shell_entries(
    mut menu: PopupMenu,
    entries: &[ShellContextMenuEntry],
    paths: &[PathBuf],
    extended_verbs: bool,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let browser = browser.clone();
    if entries_contain_submenu(entries) {
        // Native submenu rows (hover flyout); plain rows as PopupMenuItem (not inside scrollable).
        let mut flat_batch = Vec::new();
        for entry in entries {
            match entry {
                ShellContextMenuEntry::Submenu {
                    label,
                    children,
                    lazy_parent_index,
                    ..
                } => {
                    if !flat_batch.is_empty() {
                        menu =
                            append_shell_flat_popup_items(menu, &flat_batch, paths, extended_verbs);
                        flat_batch.clear();
                    }
                    menu = append_shell_submenu_item(
                        menu,
                        label.clone(),
                        children,
                        *lazy_parent_index,
                        paths,
                        extended_verbs,
                        browser.clone(),
                        window,
                        cx,
                    );
                }
                _ => flat_batch.push(entry.clone()),
            }
        }
        if !flat_batch.is_empty() {
            menu = append_shell_flat_popup_items(menu, &flat_batch, paths, extended_verbs);
        }
        apply_context_menu_style(menu.max_h(SHELL_MENU_MAX_HEIGHT))
    } else {
        for entry in entries {
            match entry {
                ShellContextMenuEntry::Separator => {
                    menu = menu.separator();
                }
                ShellContextMenuEntry::Item {
                    label,
                    command_offset,
                    command_string,
                    icon_png,
                    ..
                } => {
                    menu = menu.item(shell_popup_item(
                        label.clone(),
                        icon_png.clone(),
                        paths.to_vec(),
                        *command_offset,
                        command_string.clone(),
                        extended_verbs,
                    ));
                }
                ShellContextMenuEntry::Submenu { .. } => {}
            }
        }
        apply_context_menu_style(finish_shell_popup_menu(menu))
    }
}

fn build_recycle_item_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let can_paste = AppFileClipboard::has_items(cx);
    let state = browser.read(cx);
    let has_selection = !state.selected_paths.is_empty();
    let focus = state.focus_handle.clone();

    let mut menu = menu.action_context(focus);
    menu = append_quick_action_toolbar(menu, has_selection, can_paste);
    menu.separator()
        .item(context_menu_action_item(
            t!("files.menu.open"),
            Icon::new(IconName::Folder),
            Box::new(OpenItem),
            false,
            false,
        ))
        .separator()
        .item(context_menu_action_item(
            t!("files.menu.copy"),
            Icon::new(IconName::Copy),
            Box::new(CopyItems),
            false,
            false,
        ))
        .separator()
        .item(context_menu_action_item(
            t!("files.menu.delete_permanent"),
            Icon::new(IconName::Delete),
            Box::new(DeleteItemsPermanent),
            false,
            false,
        ))
        .item(context_menu_action_item(
            t!("files.menu.properties"),
            Icon::new(IconName::Settings2),
            Box::new(ShellProperties),
            false,
            false,
        ))
}

fn build_file_tag_item_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let state = browser.read(cx);
    let paths = state.selected_paths_vec();
    let single_dir = paths.len() == 1 && paths[0].is_dir();
    let has_selection = !paths.is_empty();
    let focus = state.focus_handle.clone();

    let mut menu = menu.action_context(focus);
    menu = append_quick_action_toolbar(menu, has_selection, false);
    menu = menu
        .separator()
        .item(context_menu_action_item(
            t!("files.menu.open"),
            Icon::new(IconName::Folder),
            Box::new(OpenItem),
            false,
            false,
        ));

    if single_dir {
        let path = paths[0].clone();
        let tab_label: SharedString = t!("sidebar.menu.open_new_tab").into();
        menu = menu.item(
            PopupMenuItem::element(move |_, _| {
                context_menu_text_row(tab_label.clone(), Some(Icon::new(IconName::File)), false)
            })
            .on_click(move |_, _, cx| {
                AppNavigation::open_path_in_new_tab(path.clone(), cx);
            }),
        );
    }

    menu.item(context_menu_action_item(
        t!("files.menu.copy_path"),
        Icon::new(IconName::ExternalLink),
        Box::new(CopyPath),
        false,
        false,
    ))
    .item(context_menu_action_item(
        t!("files.menu.properties"),
        Icon::new(IconName::Settings2),
        Box::new(ShellProperties),
        false,
        false,
    ))
}

fn path_is_pinned(path_string: &str) -> bool {
    load_config()
        .map(|c| c.pinned_folders.iter().any(|p| p == path_string))
        .unwrap_or(false)
}

impl FileBrowser {
    pub(crate) fn is_background_context_menu(&self) -> bool {
        if !self.selected_paths.is_empty() {
            return false;
        }
        matches!(
            self.browse_location,
            BrowseLocation::Directory | BrowseLocation::RecycleBin | BrowseLocation::FileTag { .. }
        )
    }

    pub(crate) fn set_context_menu_extended_verbs(&mut self, extended: bool) {
        self.context_menu_extended_verbs = extended;
        let mismatch = self
            .shell_menu_cache
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|c| c.extended_verbs != extended))
            .unwrap_or(false);
        if mismatch {
            if let Ok(mut guard) = self.shell_menu_cache.write() {
                *guard = None;
            }
            self.shell_menu_revision = self.shell_menu_revision.wrapping_add(1);
        }
    }
}
