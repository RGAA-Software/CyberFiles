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
use gpui::{div, prelude::*, Context, Entity, SharedString, Window, px};
use gpui_component::{
    h_flex,
    menu::{PopupMenu, PopupMenuItem},
    notification::Notification,
    Disableable as _, Icon, IconName, WindowExt as _,
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

/// Build the file list context menu (background or item flyout).
pub fn build_context_menu(
    menu: PopupMenu,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    if browser.read(cx).is_background_context_menu() {
        build_background_menu(menu, browser, window, cx)
    } else {
        build_item_menu(menu, browser, window, cx)
    }
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
            .item(
                PopupMenuItem::new(t!("files.view.details"))
                    .icon(Icon::new(IconName::GalleryVerticalEnd))
                    .checked(view_mode == ViewMode::Details)
                    .action(Box::new(ViewDetails)),
            )
            .item(
                PopupMenuItem::new(t!("files.view.grid"))
                    .icon(Icon::new(IconName::LayoutDashboard))
                    .checked(view_mode == ViewMode::Grid)
                    .action(Box::new(ViewGrid)),
            )
            .item(
                PopupMenuItem::new(t!("files.view.columns"))
                    .icon(Icon::new(IconName::PanelLeft))
                    .checked(view_mode == ViewMode::Columns)
                    .action(Box::new(ViewColumns)),
            )
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
            .item(
                PopupMenuItem::new(t!("files.sort.name"))
                    .checked(sort.option == SortOption::Name)
                    .action(Box::new(SortByName)),
            )
            .item(
                PopupMenuItem::new(t!("files.sort.modified"))
                    .checked(sort.option == SortOption::DateModified)
                    .action(Box::new(SortByModified)),
            )
            .item(
                PopupMenuItem::new(t!("files.sort.created"))
                    .checked(sort.option == SortOption::DateCreated)
                    .action(Box::new(SortByCreated)),
            )
            .item(
                PopupMenuItem::new(t!("files.sort.size"))
                    .checked(sort.option == SortOption::Size)
                    .action(Box::new(SortBySize)),
            )
            .item(
                PopupMenuItem::new(t!("files.sort.type"))
                    .checked(sort.option == SortOption::FileType)
                    .action(Box::new(SortByType)),
            )
            .separator()
            .menu(
                t!("files.sort.toggle_direction"),
                Box::new(ToggleSortDirection),
            )
            .menu(hidden_label, Box::new(ToggleShowHidden))
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

    menu = menu.menu_with_icon(
        t!("files.menu.open"),
        Icon::new(IconName::Folder),
        Box::new(OpenItem),
    );

    if single && !paths[0].is_dir() {
        menu = menu.menu_with_icon(
            t!("files.menu.open_with"),
            Icon::new(IconName::Settings2),
            Box::new(OpenWithDialog),
        );
    }

    if single {
        let path = paths[0].clone();
        menu = menu.item(
            PopupMenuItem::new(t!("sidebar.menu.open_new_tab"))
                .icon(Icon::new(IconName::File))
                .on_click(move |_, _, cx| {
                    AppNavigation::open_path_in_new_tab(path.clone(), cx);
                }),
        );
        menu = menu.menu_with_icon(
            t!("files.menu.open_in_new_window"),
            Icon::new(IconName::ExternalLink),
            Box::new(OpenInNewWindow),
        );
        menu = menu.menu_with_icon(
            t!("files.menu.open_in_new_pane"),
            Icon::new(IconName::PanelLeft),
            Box::new(OpenInNewPane),
        );
    }

    menu = menu.separator();

    menu = menu.menu_with_icon(
        t!("files.menu.copy_path"),
        Icon::new(IconName::ExternalLink),
        Box::new(CopyPath),
    );

    if multi {
        menu = menu.menu_with_icon(
            t!("files.menu.create_folder_from_selection"),
            Icon::new(IconName::Folder),
            Box::new(CreateFolderFromSelection),
        );
    }

    if single && !paths[0].is_dir() {
        menu = menu.menu_with_icon(
            t!("files.menu.create_shortcut"),
            Icon::new(IconName::ExternalLink),
            Box::new(CreateShortcut),
        );
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
            menu = menu.item(
                PopupMenuItem::new(pin_label)
                    .icon(pin_icon)
                    .on_click(move |_, _, cx| AppNavigation::unpin_folder(&ps, cx)),
            );
        } else if path.exists() {
            menu = menu.item(
                PopupMenuItem::new(pin_label)
                    .icon(pin_icon)
                    .on_click(move |_, _, cx| AppNavigation::pin_folder(path.clone(), cx)),
            );
        }
    }

    if single {
        if let Some(parent) = paths[0].parent() {
            let parent = parent.to_path_buf();
            menu = menu.item(
                PopupMenuItem::new(t!("files.menu.open_file_location"))
                    .icon(Icon::new(IconName::Folder))
                    .on_click(move |_, _, cx| {
                        AppNavigation::navigate_to_path(parent.clone(), cx);
                    }),
            );
        }
    }

    menu = menu
        .separator()
        .menu_with_icon(
            t!("files.menu.open_in_terminal"),
            Icon::new(IconName::File),
            Box::new(OpenInTerminal),
        )
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
                .gap(px(2.))
                .px_2()
                .py_1()
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
    menu.submenu_with_icon(
        Some(Icon::new(IconName::Ellipsis)),
        t!("files.menu.show_more_options"),
        window,
        cx,
        move |sub, window, cx| {
            match shell_submenu_snapshot(&shell_menu_cache, &paths_for_sub, extended_verbs) {
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
            )
            .scrollable(true)
            .max_h(px(420.)),
            }
        },
    )
}

fn placeholder_item(
    label: impl Into<SharedString>,
    icon: IconName,
    message: impl Into<SharedString>,
) -> PopupMenuItem {
    let message = message.into();
    PopupMenuItem::new(label)
        .icon(Icon::new(icon))
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

    let row_label = display_label.clone();
    let row_png = icon_png.map(Arc::new);
    PopupMenuItem::element(move |window, _| {
        let icon_slot = if let Some(png) = row_png.clone() {
            shell_menu_icon_img(png, window).into_any_element()
        } else {
            div().w(px(16.)).h(px(16.)).into_any_element()
        };
        h_flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .min_w(px(200.))
            .child(icon_slot)
            .child(div().text_sm().child(row_label.clone()))
            .into_any_element()
    })
    .on_click(move |_, window, cx| invoke(window, cx))
}

fn append_shell_entries(
    mut menu: PopupMenu,
    entries: &[ShellContextMenuEntry],
    paths: &[PathBuf],
    extended_verbs: bool,
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let browser = browser.clone();
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
            ShellContextMenuEntry::Submenu {
                label,
                children,
                lazy_parent_index,
                ..
            } => {
                let paths = paths.to_vec();
                let label = platform::format_shell_menu_label(&label);
                let browser_sub = browser.clone();
                let lazy_index = *lazy_parent_index;
                let children = children.clone();
                menu = menu.submenu(label, window, cx, move |sub, window, cx| {
                    let entries = if let Some(index) = lazy_index {
                        match std::thread::spawn(move || platform::load_lazy_submenu(index)).join() {
                            Ok(Ok(items)) => items,
                            Ok(Err(error)) => {
                                eprintln!("[shell-menu] lazy submenu err: {error:#}");
                                Vec::new()
                            }
                            Err(_) => Vec::new(),
                        }
                    } else {
                        children.clone()
                    };
                    if entries.is_empty() {
                        sub.item(
                            PopupMenuItem::new(t!("files.menu.shell_empty")).disabled(true),
                        )
                    } else {
                        append_shell_entries(
                            sub,
                            &entries,
                            &paths,
                            extended_verbs,
                            browser_sub.clone(),
                            window,
                            cx,
                        )
                        .scrollable(true)
                        .max_h(px(420.))
                    }
                });
            }
        }
    }
    menu
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
        .menu_with_icon(t!("files.menu.open"), Icon::new(IconName::Folder), Box::new(OpenItem))
        .separator()
        .menu_with_icon(t!("files.menu.copy"), Icon::new(IconName::Copy), Box::new(CopyItems))
        .separator()
        .menu_with_icon(
            t!("files.menu.delete_permanent"),
            Icon::new(IconName::Delete),
            Box::new(DeleteItemsPermanent),
        )
        .menu_with_icon(
            t!("files.menu.properties"),
            Icon::new(IconName::Settings2),
            Box::new(ShellProperties),
        )
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
        .menu_with_icon(t!("files.menu.open"), Icon::new(IconName::Folder), Box::new(OpenItem));

    if single_dir {
        let path = paths[0].clone();
        menu = menu.item(
            PopupMenuItem::new(t!("sidebar.menu.open_new_tab"))
                .icon(Icon::new(IconName::File))
                .on_click(move |_, _, cx| {
                    AppNavigation::open_path_in_new_tab(path.clone(), cx);
                }),
        );
    }

    menu.menu_with_icon(
        t!("files.menu.copy_path"),
        Icon::new(IconName::ExternalLink),
        Box::new(CopyPath),
    )
    .menu_with_icon(
        t!("files.menu.properties"),
        Icon::new(IconName::Settings2),
        Box::new(ShellProperties),
    )
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
