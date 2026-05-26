use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::app_state::AppFileClipboard;
use crate::app_state::AppNavigation;
use crate::file_ops::{
    spawn_compress, spawn_file_transfer, spawn_paste_from_clipboard, FileTransferKind,
};
use crate::color_icon;
use crate::icons::{
    compact_icon, folder_icon_element, toolbar_icon,
};
use crate::list_icon_cache;
use crate::popup_menu::PopupMenu;
use crate::shell::navigation::NavigationTarget;
use crate::toolbar_button::TOOLBAR_BUTTON_PX;
use crate::toolbar_button::{toolbar_dropdown_button, toolbar_icon_button, toolbar_labeled_button};
use chrono::{DateTime, Local};
use cyberfiles_commands::{
    CompressItems, CopyItems, CopyPath, CutItems, DeleteItems, DeleteItemsPermanent, FocusSearch,
    NavigateNext, NavigatePrevious, NewFile, NewFolder, OpenItem, PasteItems, RefreshDirectory,
    RenameItem, SelectAll, ShellProperties, ViewCards, ViewColumns, ViewDetails, ViewGrid,
    ViewList, FILE_BROWSER,
};
use cyberfiles_core::{
    file_sort_prefs_from_config, file_view_mode_from_config, load_config, save_file_browser_prefs,
    VIEW_CARDS, VIEW_COLUMNS, VIEW_DETAILS, VIEW_GRID, VIEW_LIST,
};
use cyberfiles_fs::{
    column_trail_for, create_directory, create_file, delete_paths, file_items_for_tag_paths,
    filter_items_by_query, home_navigation_path, move_items, read_directory, read_recycle_bin,
    recycle_paths, rename_path, unique_new_file_name, unique_new_folder_name, ClipboardOperation,
    DirectoryReadOptions, DirectoryWatcher, FileClipboard, FileItem, FileItemKind, SortDirection,
    SortOption, SortPreferences,
};
use cyberfiles_platform_windows::{self as platform, ShellContextMenuEntry};
use gpui::{
    actions, anchored, deferred, prelude::*, ClickEvent, ClipboardItem, DismissEvent, Entity,
    FocusHandle, Focusable, ParentElement, ScrollStrategy, Subscription, Window, *,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    dialog::DialogButtonProps,
    h_flex,
    input::{Input, InputState},
    notification::Notification,
    scroll::{ScrollableElement as _, ScrollbarAxis},
    v_flex, v_virtual_list, ActiveTheme as _, Disableable as _, ElementExt as _, IconName,
    Sizable as _, VirtualListScrollHandle, WindowExt as _,
};
use rust_i18n::t;

#[path = "file_browser/context_menu.rs"]
mod context_menu;

actions!(
    file_browser_prefs,
    [
        SortByName,
        SortByModified,
        SortByCreated,
        SortBySize,
        SortByType,
        ToggleSortDirection,
        ToggleShowHidden,
        ToggleShowFileExtensions,
        OpenInNewPane,
        OpenInNewWindow,
        OpenInTerminal,
        OpenWithDialog,
        CreateFolderFromSelection,
        CreateShortcut,
    ]
);

const FILE_ROW_SIZE_COMPACT: Size<Pixels> = size(px(1.), px(28.));
const FILE_ROW_SIZE: Size<Pixels> = size(px(1.), px(36.));
const FILE_ROW_SIZE_LARGE: Size<Pixels> = size(px(1.), px(44.));
const GRID_CELL_SIZE_SMALL: Size<Pixels> = size(px(96.), px(72.));
const GRID_CELL_SIZE: Size<Pixels> = size(px(112.), px(80.));
const GRID_CELL_SIZE_LARGE: Size<Pixels> = size(px(144.), px(104.));
const CARD_CELL_SIZE: Size<Pixels> = size(px(160.), px(120.));
const COLUMN_ROW_SIZE: Size<Pixels> = size(px(1.), px(32.));
const COLUMN_WIDTH: Pixels = px(200.);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Details,
    List,
    Grid,
    Cards,
    Columns,
}

impl ViewMode {
    fn from_config(value: &str) -> Self {
        match value {
            VIEW_GRID => Self::Grid,
            VIEW_CARDS => Self::Cards,
            VIEW_COLUMNS => Self::Columns,
            _ => Self::Details,
        }
    }

    fn config_value(self) -> &'static str {
        match self {
            Self::Details => VIEW_DETAILS,
            Self::List => VIEW_LIST,
            Self::Grid => VIEW_GRID,
            Self::Cards => VIEW_CARDS,
            Self::Columns => VIEW_COLUMNS,
        }
    }
}

pub use crate::drag::DraggedFilePaths;

struct DragPathPreview {
    label: SharedString,
}

impl Render for DragPathPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded(cx.theme().radius)
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().border)
            .text_sm()
            .text_color(cx.theme().popover_foreground)
            .child(self.label.clone())
    }
}

struct RenameState {
    path: PathBuf,
    input: Entity<InputState>,
}

#[derive(Clone, Debug)]
pub(crate) struct ShellMenuCache {
    paths: Vec<PathBuf>,
    extended_verbs: bool,
    entries: Vec<ShellContextMenuEntry>,
}

/// Stable cache key for multi-select (order-independent).
pub(crate) fn normalize_paths_for_shell_cache(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut normalized: Vec<PathBuf> = paths.to_vec();
    normalized.sort();
    normalized
}

pub(crate) fn shell_cache_matches_selection(
    cache_paths: &[PathBuf],
    selection: &[PathBuf],
) -> bool {
    normalize_paths_for_shell_cache(cache_paths) == normalize_paths_for_shell_cache(selection)
}

/// Shell submenu content snapshot (built when the flyout is created — no `FileBrowser::read` in submenu callbacks).
#[derive(Clone, Debug)]
pub(crate) enum ShellSubmenuSnapshot {
    Loading,
    Empty,
    Ready(Vec<platform::ShellContextMenuEntry>),
}

pub(crate) fn shell_submenu_snapshot(
    cache: &Arc<RwLock<Option<ShellMenuCache>>>,
    paths: &[PathBuf],
    extended_verbs: bool,
) -> ShellSubmenuSnapshot {
    let Ok(guard) = cache.read() else {
        return ShellSubmenuSnapshot::Loading;
    };
    let Some(cache) = guard.as_ref() else {
        return ShellSubmenuSnapshot::Loading;
    };
    if !shell_cache_matches_selection(&cache.paths, paths) {
        return ShellSubmenuSnapshot::Loading;
    }
    if cache.extended_verbs != extended_verbs {
        return ShellSubmenuSnapshot::Loading;
    }
    if cache.entries.is_empty() {
        ShellSubmenuSnapshot::Empty
    } else {
        ShellSubmenuSnapshot::Ready(cache.entries.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrowseLocation {
    Directory,
    RecycleBin,
    FileTag { tag_name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SweepSelectionSurface {
    Main,
    Column(usize),
}

#[derive(Clone)]
struct SweepSelectionState {
    surface: SweepSelectionSurface,
    start_index: Option<usize>,
    current_index: Option<usize>,
    start_position: Point<Pixels>,
    current_position: Point<Pixels>,
    base_selection: BTreeSet<PathBuf>,
    modifiers: Modifiers,
}

pub struct FileBrowser {
    focus_handle: FocusHandle,
    browse_location: BrowseLocation,
    current_dir: PathBuf,
    back_stack: Vec<PathBuf>,
    forward_stack: Vec<PathBuf>,
    items: Vec<FileItem>,
    read_options: DirectoryReadOptions,
    sort_preferences: SortPreferences,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_handle: VirtualListScrollHandle,
    grid_scroll_handle: VirtualListScrollHandle,
    cards_scroll_handle: VirtualListScrollHandle,
    error: Option<String>,
    selected_paths: BTreeSet<PathBuf>,
    anchor_index: Option<usize>,
    focused_index: Option<usize>,
    renaming: Option<RenameState>,
    show_toolbar: bool,
    /// View/sort/actions row (Files `InnerNavigationToolbar`), below window nav + omnibar.
    show_content_toolbar: bool,
    show_info_pane: bool,
    view_mode: ViewMode,
    view_size_level: u8,
    search_query: String,
    display_items: Vec<FileItem>,
    column_trail: Vec<PathBuf>,
    column_listings: Vec<Vec<FileItem>>,
    column_scroll_handles: Vec<VirtualListScrollHandle>,
    _directory_watcher: Option<DirectoryWatcher>,
    _watcher_task: Option<Task<()>>,
    watched_dir: Option<PathBuf>,
    shell_menu_cache: Arc<RwLock<Option<ShellMenuCache>>>,
    _shell_menu_task: Option<Task<()>>,
    /// Selection key for an in-flight Shell fetch (dedupe rapid right-clicks).
    shell_menu_fetch_paths: Option<Vec<PathBuf>>,
    shell_menu_fetch_generation: u64,
    context_menu_extended_verbs: bool,
    context_menu_open: bool,
    context_menu_position: Point<Pixels>,
    context_menu_view: Option<Entity<PopupMenu>>,
    _context_menu_subscription: Option<Subscription>,
    /// Bumped when Shell entries finish loading; drives menu rebuild while open.
    shell_menu_revision: u64,
    context_menu_built_revision: u64,
    /// Bumped on each `refresh`; list icons warm once per bump (not per scroll).
    list_icon_warm_token: u64,
    list_icon_warm_scheduled: u64,
    _subscriptions: Vec<Subscription>,
    /// Cached measured cells-per-row for grid view.
    grid_cells_per_row: Option<usize>,
    /// Cached measured cells-per-row for cards view.
    cards_cells_per_row: Option<usize>,
    /// Last known viewport width; used to invalidate caches on window resize.
    last_viewport_width: Option<Pixels>,
    /// Selected file in columns view (col_index, path). Folders are tracked via column_trail.
    column_selected_path: Option<(usize, PathBuf)>,
    /// Active column in columns view. Determines which column receives actions like SelectAll.
    active_column_index: Option<usize>,
    sweep_selection: Option<SweepSelectionState>,
    main_sweep_bounds: Option<Bounds<Pixels>>,
    column_sweep_bounds: Option<(usize, Bounds<Pixels>)>,
}

impl FileBrowser {
    /// File list for embedding in MainPage (window nav + omnibar live on `MainPage`).
    pub fn for_shell(cx: &mut Context<Self>, initial_dir: PathBuf) -> Self {
        Self::with_options(cx, initial_dir, false, true)
    }

    fn with_options(
        cx: &mut Context<Self>,
        current_dir: PathBuf,
        show_toolbar: bool,
        show_content_toolbar: bool,
    ) -> Self {
        let mut read_options = DirectoryReadOptions::default();
        let mut sort_preferences = SortPreferences::default();
        let (sort_option, sort_direction, show_hidden, show_file_extensions) =
            file_sort_prefs_from_config();
        {
            if let Some(option) = sort_option {
                sort_preferences.option = sort_option_from_config(&option);
            }
            if let Some(direction) = sort_direction {
                sort_preferences.direction = sort_direction_from_config(&direction);
            }
            if let Some(hidden) = show_hidden {
                read_options.show_hidden_items = hidden;
                read_options.show_dot_files = hidden;
            }
            read_options.show_file_extensions = show_file_extensions;
        }

        let view_mode = ViewMode::from_config(&file_view_mode_from_config());
        let (items, error) = load_files_dir(&current_dir, read_options, sort_preferences);
        let display_items = filter_items_by_query(&items, "");
        let column_trail = column_trail_for(&current_dir);
        let column_listings =
            column_listings_for(&column_trail, &read_options, sort_preferences, "");
        let column_scroll_handles = column_listings
            .iter()
            .map(|_| VirtualListScrollHandle::new())
            .collect();

        Self {
            focus_handle: cx.focus_handle(),
            browse_location: BrowseLocation::Directory,
            current_dir,
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            item_sizes: item_sizes_for(display_items.len(), ViewMode::Details, 2),
            scroll_handle: VirtualListScrollHandle::new(),
            grid_scroll_handle: VirtualListScrollHandle::new(),
            cards_scroll_handle: VirtualListScrollHandle::new(),
            items,
            read_options,
            sort_preferences,
            error,
            selected_paths: BTreeSet::new(),
            anchor_index: None,
            focused_index: None,
            renaming: None,
            show_toolbar,
            show_content_toolbar,
            show_info_pane: false,
            view_mode,
            view_size_level: 2,
            search_query: String::new(),
            display_items,
            column_trail,
            column_listings,
            column_scroll_handles,
            _directory_watcher: None,
            _watcher_task: None,
            watched_dir: None,
            shell_menu_cache: Arc::new(RwLock::new(None)),
            _shell_menu_task: None,
            shell_menu_fetch_paths: None,
            shell_menu_fetch_generation: 0,
            context_menu_extended_verbs: false,
            context_menu_open: false,
            context_menu_position: Point::default(),
            context_menu_view: None,
            _context_menu_subscription: None,
            shell_menu_revision: 0,
            context_menu_built_revision: 0,
            list_icon_warm_token: 0,
            list_icon_warm_scheduled: u64::MAX,
            _subscriptions: Vec::new(),
            grid_cells_per_row: None,
            cards_cells_per_row: None,
            last_viewport_width: None,
            column_selected_path: None,
            active_column_index: None,
            sweep_selection: None,
            main_sweep_bounds: None,
            column_sweep_bounds: None,
        }
    }

    pub fn set_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.search_query = query;
        self.apply_filter();
        cx.notify();
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        AppNavigation::focus_search(window, cx);
    }

    fn apply_filter(&mut self) {
        self.display_items = filter_items_by_query(&self.items, &self.search_query);
        self.item_sizes = item_sizes_for(self.display_items.len(), self.view_mode, self.view_size_level);
        if self.view_mode == ViewMode::Columns {
            self.refresh_column_listings();
        }
        self.clamp_focused_index();
    }

    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        if self.view_mode != mode {
            let was_columns = self.view_mode == ViewMode::Columns;
            self.view_mode = mode;
            self.grid_cells_per_row = None;
            self.cards_cells_per_row = None;
            self.item_sizes = item_sizes_for(self.display_items.len(), self.view_mode, self.view_size_level);
            // 切换视图时清除所有选中状态，避免跨视图残留
            self.selected_paths.clear();
            self.active_column_index = None;
            self.column_selected_path = None;
            self.focused_index = None;
            self.anchor_index = None;
            if mode == ViewMode::Columns {
                self.refresh_column_listings();
            } else if was_columns {
                self.refresh();
            }
            self.persist_prefs();
            cx.notify();
        }
    }

    fn persist_prefs(&self) {
        let _ = save_file_browser_prefs(
            self.view_mode.config_value(),
            sort_option_config_value(self.sort_preferences.option),
            sort_direction_config_value(self.sort_preferences.direction),
            self.read_options.show_hidden_items,
            self.read_options.show_file_extensions,
        );
    }

    fn increase_view_size(&mut self, cx: &mut Context<Self>) {
        if self.view_size_level < 3 {
            self.view_size_level += 1;
            self.item_sizes = item_sizes_for(self.display_items.len(), self.view_mode, self.view_size_level);
            cx.notify();
        }
    }

    fn decrease_view_size(&mut self, cx: &mut Context<Self>) {
        if self.view_size_level > 1 {
            self.view_size_level -= 1;
            self.item_sizes = item_sizes_for(self.display_items.len(), self.view_mode, self.view_size_level);
            cx.notify();
        }
    }

    pub fn set_show_info_pane(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.show_info_pane != show {
            self.show_info_pane = show;
            self.grid_cells_per_row = None;
            self.cards_cells_per_row = None;
            cx.notify();
        }
    }

    fn refresh_column_listings(&mut self) {
        self.column_trail = column_trail_for(&self.current_dir);
        self.column_listings = column_listings_for(
            &self.column_trail,
            &self.read_options,
            self.sort_preferences,
            &self.search_query,
        );
        self.column_scroll_handles = self
            .column_listings
            .iter()
            .map(|_| VirtualListScrollHandle::new())
            .collect();
    }

    /// Prefetch Shell context menu on the dedicated Shell STA worker (non-blocking for GPUI).
    fn request_shell_menu_fetch(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.browse_location != BrowseLocation::Directory {
            return;
        }

        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }

        let extended = self.context_menu_extended_verbs;
        let paths_key = normalize_paths_for_shell_cache(&paths);
        if self
            .shell_menu_cache
            .read()
            .ok()
            .and_then(|guard| {
                guard.as_ref().map(|cache| {
                    cache.paths == paths_key
                        && cache.extended_verbs == extended
                        && !cache.entries.is_empty()
                })
            })
            .unwrap_or(false)
        {
            return;
        }

        if self.shell_menu_fetch_paths.as_ref() == Some(&paths_key) {
            return;
        }

        self._shell_menu_task.take();
        self.shell_menu_fetch_paths = Some(paths_key.clone());
        self.shell_menu_fetch_generation = self.shell_menu_fetch_generation.wrapping_add(1);
        let fetch_generation = self.shell_menu_fetch_generation;
        let browser_handle = cx.weak_entity();

        let menu_icon_extract_px = platform::menu_icon_pixel_size(window.scale_factor());
        let paths_for_query = paths_key.clone();
        let paths_for_retry = paths_key.clone();
        self._shell_menu_task = Some(cx.spawn(async move |this, cx| {
            let query_result = cx
                .background_spawn(async move {
                    platform::query_shell_context_menu_items(
                        &paths_for_query,
                        extended,
                        menu_icon_extract_px,
                    )
                })
                .await;
            let retry_after_err = query_result.is_err();

            let menu_open = this
                .update(cx, |browser, cx| {
                    browser.shell_menu_fetch_paths = None;
                    if fetch_generation != browser.shell_menu_fetch_generation {
                        return false;
                    }
                    match query_result {
                        Ok(entries) => {
                            if let Ok(mut guard) = browser.shell_menu_cache.write() {
                                *guard = Some(ShellMenuCache {
                                    paths: paths_key,
                                    extended_verbs: extended,
                                    entries,
                                });
                            }
                        }
                        Err(error) => {
                            eprintln!(
                                "[shell-menu] fetch err: paths={:?} extended={} error={error:#} (not cached; will retry)",
                                paths_key, extended
                            );
                            if let Ok(mut guard) = browser.shell_menu_cache.write() {
                                *guard = None;
                            }
                        }
                    }
                    browser.shell_menu_revision = browser.shell_menu_revision.wrapping_add(1);
                    let open = browser.context_menu_open;
                    cx.notify();
                    open
                })
                .unwrap_or(false);

            if menu_open {
                let handle = browser_handle.clone();
                let _ = this.update(cx, |_, cx| {
                    cx.defer(move |cx| {
                        let Some(window) = cx.active_window() else {
                            return;
                        };
                        let _ = window.update(cx, |_, window, cx| {
                            FileBrowser::install_context_menu_flyout(&handle, window, cx, false);
                        });
                    });
                });
                if retry_after_err {
                    let paths_retry = paths_for_retry.clone();
                    let retry_handle = browser_handle.clone();
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                    let _ = this.update(cx, |browser, cx| {
                        if !browser.context_menu_open {
                            return;
                        }
                        let selection =
                            normalize_paths_for_shell_cache(&browser.selected_paths_vec());
                        if selection != paths_retry {
                            return;
                        }
                        let cache_hit = browser
                            .shell_menu_cache
                            .read()
                            .ok()
                            .map(|g| g.is_some())
                            .unwrap_or(false);
                        if cache_hit {
                            return;
                        }
                        let handle = retry_handle.clone();
                        cx.defer(move |cx| {
                            let Some(window) = cx.active_window() else {
                                return;
                            };
                            let _ = window.update(cx, |_, window, cx| {
                                let _ = handle.update(cx, |browser, cx| {
                                    browser.request_shell_menu_fetch(window, cx);
                                    cx.notify();
                                });
                            });
                        });
                    });
                    let _ = this.update(cx, |_, cx| {
                        cx.defer(move |cx| {
                            let Some(window) = cx.active_window() else {
                                return;
                            };
                            let _ = window.update(cx, |_, window, cx| {
                                FileBrowser::install_context_menu_flyout(&retry_handle, window, cx, false);
                            });
                        });
                    });
                }
            }
        }));
    }

    fn dismiss_context_menu(&mut self) {
        self.context_menu_open = false;
        self.context_menu_view = None;
        self._context_menu_subscription = None;
    }

    fn open_context_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_menu_position = position;
        self.context_menu_open = true;
        self.request_shell_menu_fetch(window, cx);
        self.schedule_context_menu_rebuild(window, cx);
        cx.notify();
    }

    /// Rebuild flyout after the current `FileBrowser` update finishes (avoids `double_lease_panic`).
    fn schedule_context_menu_rebuild(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.context_menu_open {
            return;
        }
        let browser_handle = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            Self::install_context_menu_flyout(&browser_handle, window, cx, false);
        });
    }

    /// Build `PopupMenu` outside any `FileBrowser::update`, then attach it in a short update.
    fn install_context_menu_flyout(
        browser_handle: &WeakEntity<Self>,
        window: &mut Window,
        cx: &mut App,
        only_if_revision_changed: bool,
    ) {
        let Some(browser_entity) = browser_handle.upgrade() else {
            return;
        };
        let (open, needs_rebuild) = {
            let browser = browser_entity.read(cx);
            (
                browser.context_menu_open,
                browser.context_menu_built_revision != browser.shell_menu_revision,
            )
        };
        if !open || (only_if_revision_changed && !needs_rebuild) {
            return;
        }

        let menu = PopupMenu::build(window, cx, {
            let browser_entity = browser_entity.clone();
            move |menu, window, cx| {
                context_menu::build_context_menu(menu, browser_entity, window, cx)
            }
        });

        let browser_weak = browser_entity.downgrade();
        let _ = browser_weak.update(cx, |browser, cx| {
            if !browser.context_menu_open {
                return;
            }
            let dismiss_weak = browser_weak.clone();
            browser._context_menu_subscription = Some(window.subscribe(&menu, cx, {
                move |_, _: &DismissEvent, window, cx| {
                    let _ = dismiss_weak.update(cx, |browser, cx| {
                        browser.dismiss_context_menu();
                        cx.notify();
                    });
                    window.refresh();
                }
            }));
            browser.context_menu_view = Some(menu);
            browser.context_menu_built_revision = browser.shell_menu_revision;
            cx.notify();
        });
    }

    fn render_context_menu_overlay(&self, window: &Window) -> impl IntoElement {
        let Some(menu) = self.context_menu_view.clone() else {
            return div().into_any_element();
        };
        let position = self.context_menu_position;

        deferred(
            anchored().child(
                div()
                    .w(window.bounds().size.width)
                    .h(window.bounds().size.height)
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                    .child(
                        anchored()
                            .position(position)
                            .snap_to_window_with_margin(px(8.))
                            .anchor(Anchor::TopLeft)
                            .child(menu),
                    ),
            ),
        )
        .with_priority(1)
        .into_any_element()
    }

    fn prepare_context_menu_target(&mut self, index: usize) {
        let Some(item) = self.display_items.get(index) else {
            return;
        };
        let path = item.path.clone();
        if !self.selected_paths.contains(&path) {
            self.selected_paths.clear();
            self.selected_paths.insert(path);
            self.anchor_index = Some(index);
            self.focused_index = Some(index);
        }
    }

    fn prepare_column_context_menu_target(&mut self, col_index: usize, index: usize) {
        let Some(item) = self
            .column_listings
            .get(col_index)
            .and_then(|items| items.get(index))
        else {
            return;
        };

        let path = item.path.clone();
        if !self.selected_paths.contains(&path) {
            self.selected_paths.clear();
            self.selected_paths.insert(path.clone());
            self.column_selected_path = Some((col_index, path));
            self.anchor_index = Some(index);
            self.focused_index = Some(index);
        }
        self.active_column_index = Some(col_index);
    }

    fn restart_directory_watcher(&mut self, cx: &mut Context<Self>) {
        self._watcher_task.take();
        self._directory_watcher.take();

        if self.browse_location != BrowseLocation::Directory {
            return;
        }

        let Ok((watcher, events)) =
            DirectoryWatcher::watch(&self.current_dir, Duration::from_millis(300))
        else {
            return;
        };

        self._directory_watcher = Some(watcher);
        self._watcher_task = Some(cx.spawn(async move |browser, cx| {
            while events.recv_async().await.is_ok() {
                let _ = browser.update(cx, |browser, cx| {
                    browser.refresh();
                    cx.notify();
                });
            }
        }));
    }

    pub fn read_options(&self) -> &DirectoryReadOptions {
        &self.read_options
    }

    pub fn current_directory(&self) -> &PathBuf {
        &self.current_dir
    }

    pub fn navigation_target(&self) -> NavigationTarget {
        match &self.browse_location {
            BrowseLocation::Directory => NavigationTarget::Path(self.current_dir.clone()),
            BrowseLocation::RecycleBin => NavigationTarget::RecycleBin,
            BrowseLocation::FileTag { tag_name } => NavigationTarget::FileTag(tag_name.clone()),
        }
    }

    pub fn item_count(&self) -> usize {
        self.display_items.len()
    }

    pub fn selected_count(&self) -> usize {
        self.selected_paths.len()
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    pub fn can_go_up(&self) -> bool {
        self.current_dir.parent().is_some()
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        self.navigate_back(cx);
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        self.navigate_forward(cx);
    }

    pub fn go_up(&mut self, cx: &mut Context<Self>) {
        self.navigate_parent(cx);
    }

    pub fn reload(&mut self) {
        self.refresh();
    }

    pub fn open_directory_reset_history(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.browse_location = BrowseLocation::Directory;
        self.back_stack.clear();
        self.forward_stack.clear();
        self.current_dir = path;
        self.clear_selection();
        self.refresh();
        self.restart_directory_watcher(cx);
        cx.notify();
    }

    pub fn open_file_tag(&mut self, tag_name: String, cx: &mut Context<Self>) {
        self.clear_shell_menu_cache();
        self.browse_location = BrowseLocation::FileTag {
            tag_name: tag_name.clone(),
        };
        self.back_stack.clear();
        self.forward_stack.clear();
        self.current_dir = home_navigation_path();
        self.clear_selection();
        self._watcher_task.take();
        self._directory_watcher.take();
        self.watched_dir = None;
        self.refresh();
        Self::emit_location_changed(cx);
        cx.notify();
    }

    pub fn open_recycle_bin(&mut self, cx: &mut Context<Self>) {
        self.clear_shell_menu_cache();
        self.browse_location = BrowseLocation::RecycleBin;
        self.back_stack.clear();
        self.forward_stack.clear();
        self.current_dir = platform::recycle_bin_folder().unwrap_or_else(home_navigation_path);
        self.clear_selection();
        self._watcher_task.take();
        self._directory_watcher.take();
        self.watched_dir = None;
        self.refresh();
        cx.notify();
    }

    fn emit_location_changed(cx: &mut Context<Self>) {
        cx.notify();
        crate::app_state::AppNavigation::location_changed(cx);
    }

    fn refresh(&mut self) {
        let (items, error) = match &self.browse_location {
            BrowseLocation::Directory => {
                load_files_dir(&self.current_dir, self.read_options, self.sort_preferences)
            }
            BrowseLocation::RecycleBin => {
                match read_recycle_bin(self.read_options, self.sort_preferences) {
                    Ok(items) => (items, None),
                    Err(error) => (Vec::new(), Some(error.to_string())),
                }
            }
            BrowseLocation::FileTag { tag_name } => {
                let paths = paths_for_file_tag(tag_name);
                if paths.is_empty() {
                    (Vec::new(), Some(t!("file_tag.empty").to_string()))
                } else {
                    (
                        file_items_for_tag_paths(&paths, self.read_options, self.sort_preferences),
                        None,
                    )
                }
            }
        };
        self.items = items;
        self.error = error;
        self.apply_filter();
        if self.view_mode == ViewMode::Columns && self.browse_location == BrowseLocation::Directory
        {
            self.refresh_column_listings();
        }
        self.reconcile_selection();
        self.clamp_focused_index();
        self.list_icon_warm_token = self.list_icon_warm_token.wrapping_add(1);
    }

    fn handle_drop(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        let dest = self.current_dir.clone();
        if paths.iter().all(|p| p.parent() == Some(dest.as_path())) {
            return;
        }
        let copy = window.modifiers().control;
        let kind = if copy {
            FileTransferKind::Copy
        } else {
            FileTransferKind::Move
        };
        let browser = cx.entity();
        spawn_file_transfer(browser, window, cx, kind, paths, dest);
    }

    fn drag_paths_for_item(&self, _index: usize, path: &Path) -> Vec<PathBuf> {
        if self.selected_paths.contains(path) && !self.selected_paths.is_empty() {
            return self.selected_paths_vec();
        }
        vec![path.to_path_buf()]
    }

    fn select_column_item(&mut self, col_index: usize, item: &FileItem, cx: &mut Context<Self>) {
        self.active_column_index = Some(col_index);
        match item.kind {
            FileItemKind::Folder => {
                if self.current_dir != item.path {
                    self.back_stack.push(self.current_dir.clone());
                    self.forward_stack.clear();
                }
                self.current_dir = item.path.clone();
                self.column_trail.truncate(col_index + 1);
                self.column_trail.push(item.path.clone());
                self.column_selected_path = None;
                self.clear_selection();
                self.refresh();
                Self::emit_location_changed(cx);
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                self.open_item(item.path.clone(), item.kind, cx);
            }
        }
    }

    fn activate_column(&mut self, col_index: usize, cx: &mut Context<Self>) {
        let Some(path) = self.column_trail.get(col_index).cloned() else {
            return;
        };

        self.active_column_index = Some(col_index);
        self.current_dir = path;
        Self::emit_location_changed(cx);
        cx.notify();
    }

    fn column_selection_name(&self, col_index: usize) -> Option<String> {
        let next = self.column_trail.get(col_index + 1)?;
        next.file_name().map(|n| n.to_string_lossy().to_string())
    }

    fn clear_shell_menu_cache(&mut self) {
        platform::clear_shell_menu_session();
        if let Ok(mut guard) = self.shell_menu_cache.write() {
            *guard = None;
        }
        self._shell_menu_task.take();
        self.shell_menu_fetch_paths = None;
    }

    fn navigate_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !matches!(self.browse_location, BrowseLocation::Directory) {
            self.browse_location = BrowseLocation::Directory;
        }
        if path == self.current_dir {
            return;
        }

        self.clear_shell_menu_cache();
        self.back_stack.push(self.current_dir.clone());
        self.forward_stack.clear();
        self.current_dir = path;
        self.clear_selection();
        self.refresh();
        Self::emit_location_changed(cx);
    }

    fn navigate_back(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.back_stack.pop() else {
            return;
        };

        self.forward_stack.push(self.current_dir.clone());
        self.current_dir = path;
        self.clear_selection();
        self.refresh();
        Self::emit_location_changed(cx);
    }

    fn navigate_forward(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.forward_stack.pop() else {
            return;
        };

        self.back_stack.push(self.current_dir.clone());
        self.current_dir = path;
        self.clear_selection();
        self.refresh();
        Self::emit_location_changed(cx);
    }

    fn navigate_parent(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.current_dir.parent() {
            self.navigate_to(parent.to_path_buf(), cx);
        }
    }

    fn clear_selection(&mut self) {
        self.selected_paths.clear();
        self.anchor_index = None;
        self.focused_index = None;
        self.column_selected_path = None;
    }

    fn begin_sweep_selection(
        &mut self,
        surface: SweepSelectionSurface,
        start_position: Point<Pixels>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let start_index = if modifiers.shift {
            match surface {
                SweepSelectionSurface::Main => self.anchor_index,
                SweepSelectionSurface::Column(col_index) => self
                    .anchor_index
                    .or_else(|| self.implicit_column_selected_index(col_index)),
            }
        } else {
            None
        };
        let base_selection = match surface {
            SweepSelectionSurface::Main => self.selected_paths.clone(),
            SweepSelectionSurface::Column(col_index) => {
                if self.selected_paths.is_empty() {
                    self.implicit_column_base_selection(col_index)
                } else {
                    self.selected_paths.clone()
                }
            }
        };
        self.sweep_selection = Some(SweepSelectionState {
            surface: surface.clone(),
            start_index,
            current_index: None,
            start_position,
            current_position: start_position,
            base_selection,
            modifiers,
        });

        match surface {
            SweepSelectionSurface::Main => {
                self.active_column_index = None;
                if !modifiers.secondary() && !modifiers.shift {
                    self.clear_selection();
                }
            }
            SweepSelectionSurface::Column(col_index) => {
                self.active_column_index = Some(col_index);
                self.column_selected_path = None;
                if !modifiers.secondary() && !modifiers.shift {
                    self.selected_paths.clear();
                }
            }
        }
        cx.notify();
    }

    fn update_sweep_pointer(
        &mut self,
        surface: SweepSelectionSurface,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.sweep_selection.as_mut() else {
            return;
        };
        if state.surface != surface || state.current_position == position {
            return;
        }
        state.current_position = position;
        if surface == SweepSelectionSurface::Main {
            self.update_main_sweep_selection_from_rect(cx);
        } else if let SweepSelectionSurface::Column(col_index) = surface {
            self.update_column_sweep_selection_from_rect(col_index, cx);
        }
        cx.notify();
    }

    fn update_sweep_selection(
        &mut self,
        surface: SweepSelectionSurface,
        hover_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.sweep_selection.as_mut() else {
            return;
        };
        if state.surface != surface {
            return;
        }
        if state.start_index.is_none() {
            state.start_index = Some(hover_index);
        }
        if state.current_index == Some(hover_index) {
            return;
        }
        state.current_index = Some(hover_index);

        if state.surface == SweepSelectionSurface::Main {
            self.update_main_sweep_selection_from_rect(cx);
            return;
        } else if let SweepSelectionSurface::Column(col_index) = state.surface {
            self.update_column_sweep_selection_from_rect(col_index, cx);
            return;
        }

        let anchor = state.start_index.unwrap_or(hover_index);
        let (start, end) = if anchor <= hover_index {
            (anchor, hover_index)
        } else {
            (hover_index, anchor)
        };

        let items: Vec<PathBuf> = match state.surface {
            SweepSelectionSurface::Main => self
                .display_items
                .get(start..=end)
                .unwrap_or(&[])
                .iter()
                .map(|item| item.path.clone())
                .collect(),
            SweepSelectionSurface::Column(col_index) => self
                .column_listings
                .get(col_index)
                .and_then(|items| items.get(start..=end))
                .unwrap_or(&[])
                .iter()
                .map(|item| item.path.clone())
                .collect(),
        };

        let mut selected_paths = if state.modifiers.secondary() {
            state.base_selection.clone()
        } else {
            BTreeSet::new()
        };

        if state.modifiers.secondary() {
            for path in items {
                if !selected_paths.insert(path.clone()) {
                    selected_paths.remove(&path);
                }
            }
        } else {
            selected_paths.extend(items);
        }

        self.selected_paths = selected_paths;
        self.focused_index = Some(hover_index);
        self.anchor_index = Some(anchor);
        if let SweepSelectionSurface::Column(col_index) = surface {
            self.active_column_index = Some(col_index);
            self.column_selected_path = None;
        }
        cx.notify();
    }

    fn finish_sweep_selection(&mut self) {
        self.sweep_selection = None;
    }

    fn update_main_sweep_selection_from_rect(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.sweep_selection.as_ref() else {
            return;
        };
        if state.surface != SweepSelectionSurface::Main {
            return;
        }
        let Some(bounds) = self.main_sweep_bounds else {
            return;
        };

        let selection_rect = self.main_sweep_rect(bounds);
        let hit_indices = self.main_sweep_hit_indices(selection_rect);
        let hit_paths = hit_indices
            .into_iter()
            .filter_map(|index| self.display_items.get(index).map(|item| item.path.clone()))
            .collect::<Vec<_>>();

        let mut selected_paths = if state.modifiers.secondary() {
            state.base_selection.clone()
        } else {
            BTreeSet::new()
        };

        if state.modifiers.secondary() {
            for path in hit_paths {
                if !selected_paths.insert(path.clone()) {
                    selected_paths.remove(&path);
                }
            }
        } else {
            selected_paths.extend(hit_paths);
        }

        self.selected_paths = selected_paths;
        self.focused_index = None;
        cx.notify();
    }

    fn update_column_sweep_selection_from_rect(&mut self, col_index: usize, cx: &mut Context<Self>) {
        let Some(state) = self.sweep_selection.as_ref() else {
            return;
        };
        if state.surface != SweepSelectionSurface::Column(col_index) {
            return;
        }
        let Some(bounds) = self
            .column_sweep_bounds
            .and_then(|(index, bounds)| (index == col_index).then_some(bounds))
        else {
            return;
        };

        let selection_rect = self.sweep_rect_in_bounds(bounds);
        let scroll_y = self
            .column_scroll_handles
            .get(col_index)
            .map(|handle| handle.offset().y)
            .unwrap_or(px(0.));
        let row_h = COLUMN_ROW_SIZE.height;
        let hit_paths = self
            .column_listings
            .get(col_index)
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(index, item)| {
                let row_rect = Bounds::new(
                    point(bounds.left(), bounds.top() + row_h * index - scroll_y),
                    size(bounds.size.width, row_h),
                );
                rects_intersect(selection_rect, row_rect).then_some(item.path.clone())
            })
            .collect::<Vec<_>>();

        let mut selected_paths = if state.modifiers.secondary() {
            state.base_selection.clone()
        } else {
            BTreeSet::new()
        };

        if state.modifiers.secondary() {
            for path in hit_paths {
                if !selected_paths.insert(path.clone()) {
                    selected_paths.remove(&path);
                }
            }
        } else {
            selected_paths.extend(hit_paths);
        }

        self.selected_paths = selected_paths;
        self.active_column_index = Some(col_index);
        self.column_selected_path = None;
        self.focused_index = None;
        cx.notify();
    }

    fn main_sweep_rect(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        self.sweep_rect_in_bounds(bounds)
    }

    fn sweep_rect_in_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let state = self
            .sweep_selection
            .as_ref()
            .expect("sweep_rect_in_bounds called without sweep selection");
        let start = clamp_point_to_bounds(state.start_position, bounds);
        let current = clamp_point_to_bounds(state.current_position, bounds);
        let origin = point(start.x.min(current.x), start.y.min(current.y));
        let size = size(
            (start.x.max(current.x) - start.x.min(current.x)).max(px(1.)),
            (start.y.max(current.y) - start.y.min(current.y)).max(px(1.)),
        );
        Bounds::new(origin, size)
    }

    fn main_sweep_hit_indices(&self, selection_rect: Bounds<Pixels>) -> Vec<usize> {
        match self.view_mode {
            ViewMode::Details | ViewMode::List => self.main_list_sweep_hit_indices(selection_rect),
            ViewMode::Grid => self.main_grid_sweep_hit_indices(selection_rect),
            ViewMode::Cards => self.main_cards_sweep_hit_indices(selection_rect),
            ViewMode::Columns => Vec::new(),
        }
    }

    fn main_list_sweep_hit_indices(&self, selection_rect: Bounds<Pixels>) -> Vec<usize> {
        let Some(bounds) = self.main_sweep_bounds else {
            return Vec::new();
        };
        let header_h = px(32.);
        let row_h = self
            .item_sizes
            .first()
            .map(|size| size.height)
            .unwrap_or(FILE_ROW_SIZE.height);
        let scroll_y = self.scroll_handle.offset().y;

        self.display_items
            .iter()
            .enumerate()
            .filter_map(|(index, _)| {
                let row_rect = Bounds::new(
                    point(
                        bounds.left(),
                        bounds.top() + header_h + row_h * index - scroll_y,
                    ),
                    size(bounds.size.width, row_h),
                );
                rects_intersect(selection_rect, row_rect).then_some(index)
            })
            .collect()
    }

    fn render_column_sweep_overlay(
        &self,
        col_index: usize,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.sweep_selection.as_ref()?;
        if state.surface != SweepSelectionSurface::Column(col_index) {
            return None;
        }
        let bounds = self
            .column_sweep_bounds
            .and_then(|(index, bounds)| (index == col_index).then_some(bounds))?;
        let selection_rect = self.sweep_rect_in_bounds(bounds);

        Some(
            div()
                .id(("files-column-sweep-selection-overlay", col_index))
                .absolute()
                .left(selection_rect.left() - bounds.left())
                .top(selection_rect.top() - bounds.top())
                .w(selection_rect.size.width)
                .h(selection_rect.size.height)
                .border_1()
                .border_color(cx.theme().primary)
                .bg(cx.theme().primary.opacity(0.18))
                .into_any_element(),
        )
    }

    fn main_grid_sweep_hit_indices(&self, selection_rect: Bounds<Pixels>) -> Vec<usize> {
        let Some(bounds) = self.main_sweep_bounds else {
            return Vec::new();
        };
        let (cell_w, cell_h) = match self.view_size_level {
            1 => (px(96.), px(72.)),
            3 => (px(144.), px(104.)),
            _ => (px(112.), px(80.)),
        };
        let gap = px(8.);
        let padding = px(8.);
        let scroll_y = self.grid_scroll_handle.offset().y;
        let cells_per_row = self.grid_cells_per_row.unwrap_or(1).max(1);

        self.display_items
            .iter()
            .enumerate()
            .filter_map(|(index, _)| {
                let row = index / cells_per_row;
                let col = index % cells_per_row;
                let item_rect = Bounds::new(
                    point(
                        bounds.left() + padding + (cell_w + gap) * col,
                        bounds.top() + padding + (cell_h + gap) * row - scroll_y,
                    ),
                    size(cell_w, cell_h),
                );
                rects_intersect(selection_rect, item_rect).then_some(index)
            })
            .collect()
    }

    fn main_cards_sweep_hit_indices(&self, selection_rect: Bounds<Pixels>) -> Vec<usize> {
        let Some(bounds) = self.main_sweep_bounds else {
            return Vec::new();
        };
        let cell_w = px(160.);
        let cell_h = px(120.);
        let gap = px(8.);
        let padding = px(8.);
        let scroll_y = self.cards_scroll_handle.offset().y;
        let cells_per_row = self.cards_cells_per_row.unwrap_or(1).max(1);

        self.display_items
            .iter()
            .enumerate()
            .filter_map(|(index, _)| {
                let row = index / cells_per_row;
                let col = index % cells_per_row;
                let item_rect = Bounds::new(
                    point(
                        bounds.left() + padding + (cell_w + gap) * col,
                        bounds.top() + padding + (cell_h + gap) * row - scroll_y,
                    ),
                    size(cell_w, cell_h),
                );
                rects_intersect(selection_rect, item_rect).then_some(index)
            })
            .collect()
    }

    fn render_main_sweep_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.sweep_selection.as_ref()?;
        if state.surface != SweepSelectionSurface::Main {
            return None;
        }
        let bounds = self.main_sweep_bounds?;
        let selection_rect = self.main_sweep_rect(bounds);
        let left = selection_rect.left() - bounds.origin.x;
        let top = selection_rect.top() - bounds.origin.y;
        let width = selection_rect.size.width;
        let height = selection_rect.size.height;

        Some(
            div()
                .id("files-sweep-selection-overlay")
                .absolute()
                .left(left)
                .top(top)
                .w(width)
                .h(height)
                .border_1()
                .border_color(cx.theme().primary)
                .bg(cx.theme().primary.opacity(0.18))
                .into_any_element(),
        )
    }

    fn handle_column_item_click(
        &mut self,
        col_index: usize,
        index: usize,
        item: &FileItem,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let path = item.path.clone();
        self.active_column_index = Some(col_index);

        if modifiers.shift {
            let anchor = self
                .anchor_index
                .or_else(|| self.implicit_column_selected_index(col_index))
                .unwrap_or(index);
            let (start, end) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };
            self.selected_paths.clear();
            if let Some(items) = self.column_listings.get(col_index) {
                for i in start..=end {
                    if let Some(item) = items.get(i) {
                        self.selected_paths.insert(item.path.clone());
                    }
                }
            }
            self.column_selected_path = None;
            self.focused_index = Some(index);
            return;
        }

        if modifiers.secondary() {
            if self.selected_paths.is_empty() {
                self.selected_paths = self.implicit_column_base_selection(col_index);
            }
            if self.selected_paths.contains(&path) {
                self.selected_paths.remove(&path);
            } else {
                self.selected_paths.insert(path);
            }
            self.column_selected_path = None;
            self.anchor_index = Some(index);
            self.focused_index = Some(index);
            return;
        }

        self.anchor_index = Some(index);
        self.focused_index = Some(index);
        match item.kind {
            FileItemKind::Folder => {
                self.select_column_item(col_index, item, cx);
                self.anchor_index = Some(index);
                self.focused_index = Some(index);
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                self.column_selected_path = Some((col_index, item.path.clone()));
                self.selected_paths.clear();
                self.selected_paths.insert(item.path.clone());
                self.column_trail.truncate(col_index + 1);
                self.column_listings = column_listings_for(
                    &self.column_trail,
                    &self.read_options,
                    self.sort_preferences,
                    &self.search_query,
                );
                self.column_scroll_handles.truncate(self.column_listings.len());
            }
        }
    }

    fn implicit_column_selected_index(&self, col_index: usize) -> Option<usize> {
        let selected_path = self.column_trail.get(col_index + 1)?;
        self.column_listings
            .get(col_index)?
            .iter()
            .position(|item| item.path == *selected_path)
    }

    fn implicit_column_base_selection(&self, col_index: usize) -> BTreeSet<PathBuf> {
        let mut base = BTreeSet::new();
        if let Some(index) = self.implicit_column_selected_index(col_index) {
            if let Some(item) = self.column_listings.get(col_index).and_then(|items| items.get(index)) {
                base.insert(item.path.clone());
            }
        }
        base
    }

    fn handle_row_click(&mut self, index: usize, event: &ClickEvent, _cx: &mut Context<Self>) {
        let Some(item) = self.display_items.get(index) else {
            return;
        };
        let path = item.path.clone();
        let modifiers = event.modifiers();

        if modifiers.shift {
            let anchor = self.anchor_index.unwrap_or(index);
            let (start, end) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };
            self.selected_paths.clear();
            for i in start..=end {
                if let Some(item) = self.display_items.get(i) {
                    self.selected_paths.insert(item.path.clone());
                }
            }
        } else if modifiers.secondary() {
            if self.selected_paths.contains(&path) {
                self.selected_paths.remove(&path);
            } else {
                self.selected_paths.insert(path.clone());
            }
            self.anchor_index = Some(index);
        } else {
            self.selected_paths.clear();
            self.selected_paths.insert(path);
            self.anchor_index = Some(index);
        }

        self.focused_index = Some(index);
    }

    fn open_item(&mut self, path: PathBuf, kind: FileItemKind, cx: &mut Context<Self>) {
        match kind {
            FileItemKind::Folder => {
                if matches!(self.browse_location, BrowseLocation::FileTag { .. }) {
                    AppNavigation::navigate_to_path(path, cx);
                    return;
                }
                self.navigate_to(path, cx);
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                if let Err(error) = open_with_system(&path) {
                    self.error = Some(error.to_string());
                }
            }
        }
    }

    fn open_focused(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.focused_index else {
            return;
        };
        let Some(item) = self.display_items.get(index) else {
            return;
        };
        self.open_item(item.path.clone(), item.kind, cx);
    }

    fn reconcile_selection(&mut self) {
        self.selected_paths
            .retain(|path| self.display_items.iter().any(|item| &item.path == path));
        if let Some(index) = self.focused_index {
            if index >= self.display_items.len() {
                self.focused_index = None;
            }
        }
    }

    fn file_item_kind_icon(kind: FileItemKind) -> AnyElement {
        match kind {
            FileItemKind::Folder => folder_icon_element(),
            FileItemKind::Symlink => compact_icon(IconName::ExternalLink).into_any_element(),
            FileItemKind::File | FileItemKind::Other => {
                compact_icon(IconName::File).into_any_element()
            }
        }
    }

    /// List row icon: custom colored SVG → Shell PNG → GPUI fallback.
    fn row_list_icon(item: &FileItem, logical_size: Pixels, window: &Window) -> impl IntoElement {
        if item.kind == FileItemKind::Folder {
            return div()
                .size(logical_size)
                .flex()
                .items_center()
                .justify_center()
                .child(folder_icon_element())
                .into_any_element();
        }
        // Prefer app-bundled colored SVGs for known file types.
        if let Some(ext) = item.extension.as_deref().filter(|e| !e.is_empty()) {
            if let Some(path) = list_icon_cache::extension_svg_path(ext) {
                return color_icon::color_icon_box(path, logical_size);
            }
        }
        let px = platform::shell_icon_pixel_size(logical_size.as_f32(), window.scale_factor());
        let key = list_icon_cache::list_icon_key(item);
        if let Some(png) = list_icon_cache::list_icon_png_cached(&key, px) {
            if !png.is_empty() {
                return img(std::sync::Arc::new(Image::from_bytes(
                    ImageFormat::Png,
                    (*png).clone(),
                )))
                .size(logical_size)
                .object_fit(ObjectFit::Contain)
                .into_any_element();
            }
        }
        div()
            .size(logical_size)
            .flex()
            .items_center()
            .justify_center()
            .child(Self::file_item_kind_icon(item.kind))
            .into_any_element()
    }

    /// After directory refresh: load at most one Shell icon per category (folder, zip, exe, …).
    fn schedule_list_icon_warm(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.list_icon_warm_scheduled == self.list_icon_warm_token {
            return;
        }
        self.list_icon_warm_scheduled = self.list_icon_warm_token;
        let keys = list_icon_cache::list_icon_keys_for_items(&self.display_items);
        let px = platform::shell_icon_pixel_size(16., window.scale_factor());
        cx.spawn(async move |this, cx| {
            let _ = cx
                .background_spawn(async move {
                    list_icon_cache::warm_list_icons(keys, px);
                })
                .await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    fn clamp_focused_index(&mut self) {
        if self.display_items.is_empty() {
            self.focused_index = None;
            return;
        }
        if let Some(index) = self.focused_index {
            if index >= self.display_items.len() {
                self.focused_index = Some(self.display_items.len() - 1);
            }
        }
    }

    fn move_focus(&mut self, delta: isize) {
        if self.display_items.is_empty() {
            return;
        }
        let index = self.focused_index.unwrap_or(0);
        let next =
            (index as isize + delta).clamp(0, self.display_items.len() as isize - 1) as usize;
        self.focused_index = Some(next);
        self.scroll_handle
            .scroll_to_item(next, ScrollStrategy::Center);
    }

    pub fn select_all(&mut self) {
        if self.view_mode == ViewMode::Columns {
            let col_index = self
                .active_column_index
                .unwrap_or_else(|| self.column_listings.len().saturating_sub(1));
            if let Some(items) = self.column_listings.get(col_index) {
                self.selected_paths = items.iter().map(|item| item.path.clone()).collect();
                self.column_selected_path = None;
            } else {
                self.selected_paths.clear();
                self.column_selected_path = None;
            }
        } else {
            self.selected_paths = self
                .display_items
                .iter()
                .map(|item| item.path.clone())
                .collect();
        }
        if let Some(index) = self.focused_index {
            self.anchor_index = Some(index);
        } else if !self.display_items.is_empty() {
            self.anchor_index = Some(0);
            self.focused_index = Some(0);
        }
    }

    pub fn primary_selected_item(&self) -> Option<&FileItem> {
        if self.selected_paths.len() != 1 {
            return None;
        }
        let path = self.selected_paths.iter().next()?;
        self.display_items
            .iter()
            .find(|item| &item.path == path)
            .or_else(|| {
                self.column_listings
                    .iter()
                    .flat_map(|list| list.iter())
                    .find(|item| &item.path == path)
            })
    }

    fn primary_path(&self) -> Option<PathBuf> {
        self.primary_selected_item().map(|item| item.path.clone())
    }

    fn selected_paths_vec(&self) -> Vec<PathBuf> {
        self.selected_paths.iter().cloned().collect()
    }

    fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.primary_path() else {
            return;
        };
        let default_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(default_name));
        self.renaming = Some(RenameState { path, input });
    }

    fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = self.renaming.take() else {
            return;
        };
        let new_name = renaming.input.read(cx).value();
        match rename_path(&renaming.path, &new_name) {
            Ok(target) => {
                self.error = None;
                if self.selected_paths.remove(&renaming.path) {
                    self.selected_paths.insert(target);
                }
                self.refresh();
                window.push_notification(Notification::success(t!("files.rename.success")), cx);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.renaming = Some(renaming);
            }
        }
    }

    fn cancel_rename(&mut self) {
        self.renaming = None;
    }

    fn create_folder_from_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        let name = unique_new_folder_name(&self.current_dir);
        match create_directory(&self.current_dir, &name) {
            Ok(dest_dir) => match move_items(&paths, &dest_dir) {
                Ok(()) => {
                    self.error = None;
                    self.refresh();
                    window.push_notification(
                        Notification::success(t!("files.create_folder_from_selection.success")),
                        cx,
                    );
                }
                Err(error) => self.error = Some(error.to_string()),
            },
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn create_new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = unique_new_folder_name(&self.current_dir);
        match create_directory(&self.current_dir, &name) {
            Ok(path) => {
                self.error = None;
                self.refresh();
                if let Some(index) = self.display_items.iter().position(|item| item.path == path) {
                    self.focused_index = Some(index);
                    self.selected_paths.clear();
                    self.selected_paths.insert(path);
                    self.anchor_index = self.focused_index;
                    self.begin_rename(window, cx);
                } else {
                    window.push_notification(
                        Notification::success(t!("files.new_folder.success")),
                        cx,
                    );
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn create_new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = unique_new_file_name(&self.current_dir);
        match create_file(&self.current_dir, &name) {
            Ok(path) => {
                self.error = None;
                self.refresh();
                if let Some(index) = self.display_items.iter().position(|item| item.path == path) {
                    self.focused_index = Some(index);
                    self.selected_paths.clear();
                    self.selected_paths.insert(path);
                    self.anchor_index = self.focused_index;
                    self.begin_rename(window, cx);
                } else {
                    window
                        .push_notification(Notification::success(t!("files.new_file.success")), cx);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn copy_paths(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        let text = paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    pub fn copy_items(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        AppFileClipboard::store(ClipboardOperation::Copy, paths, cx);
    }

    pub fn cut_items(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        AppFileClipboard::store(ClipboardOperation::Cut, paths, cx);
    }

    fn compress_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        let destination = self.current_directory().clone();
        spawn_compress(cx.entity(), window, cx, paths, destination);
    }

    pub fn paste_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let clipboard = AppFileClipboard::take(cx).or_else(|| {
            let paths = platform::read_clipboard_file_paths();
            (!paths.is_empty()).then(|| FileClipboard::new(ClipboardOperation::Copy, paths))
        });
        let Some(clipboard) = clipboard else {
            return;
        };
        if clipboard.paths.is_empty() {
            return;
        }

        let destination = self.current_directory().clone();
        let browser = cx.entity();
        spawn_paste_from_clipboard(browser, window, cx, clipboard, destination);
    }

    fn confirm_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete_inner(window, cx, false);
    }

    fn confirm_delete_permanent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete_inner(window, cx, true);
    }

    fn confirm_delete_inner(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        permanent: bool,
    ) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }

        let count = paths.len();
        let description = SharedString::from(if count == 1 {
            paths[0].display().to_string()
        } else {
            t!("files.delete.description_many", count = count).to_string()
        });
        let paths = paths.clone();
        let browser = cx.entity();
        let title = SharedString::from(if permanent {
            t!("files.delete_permanent.title")
        } else {
            t!("files.delete.title")
        });
        let confirm = SharedString::from(if permanent {
            t!("files.delete_permanent.confirm")
        } else {
            t!("files.delete.confirm")
        });
        let success = SharedString::from(if permanent {
            t!("files.delete_permanent.success")
        } else {
            t!("files.delete.success")
        });

        window.open_alert_dialog(cx, move |alert, _window, _cx| {
            let paths = paths.clone();
            let browser = browser.clone();
            let title = title.clone();
            let confirm = confirm.clone();
            let success = success.clone();
            alert
                .title(title)
                .description(description.clone())
                .button_props(
                    DialogButtonProps::default()
                        .ok_variant(gpui_component::button::ButtonVariant::Danger)
                        .ok_text(confirm)
                        .cancel_text(SharedString::from(t!("files.cancel")))
                        .show_cancel(true),
                )
                .on_ok(move |_dialog, _window, cx| {
                    let browser = browser.clone();
                    let success = success.clone();
                    let paths = paths.clone();
                    cx.spawn(async move |cx| {
                        let delete_result = cx
                            .background_spawn(async move {
                                if permanent {
                                    delete_paths(&paths)
                                } else {
                                    recycle_paths(&paths)
                                }
                            })
                            .await;

                        let _ = browser.update(cx, |browser, cx| {
                            let Some(window) = cx.active_window() else {
                                if delete_result.is_ok() {
                                    browser.clear_selection();
                                    browser.refresh();
                                }
                                cx.notify();
                                return;
                            };

                            let _ = window.update(cx, |_, window, cx| match &delete_result {
                                Ok(()) => {
                                    browser.clear_selection();
                                    browser.refresh();
                                    window.push_notification(
                                        Notification::success(success.clone()),
                                        cx,
                                    );
                                }
                                Err(error) => {
                                    window.push_notification(
                                        Notification::error(format!(
                                            "{}: {error}",
                                            t!("files.delete.error")
                                        )),
                                        cx,
                                    );
                                }
                            });
                            cx.notify();
                        });

                        Ok::<(), anyhow::Error>(())
                    })
                    .detach();
                    true
                })
        });
    }

    fn perform_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete(window, cx);
    }

    fn perform_delete_permanent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete_permanent(window, cx);
    }

    fn set_sort_option(&mut self, option: SortOption) {
        self.sort_preferences.option = option;
        self.refresh();
        self.persist_prefs();
    }

    fn sort_label(&self) -> String {
        let field = match self.sort_preferences.option {
            SortOption::Name => t!("files.sort.name"),
            SortOption::DateModified => t!("files.sort.modified"),
            SortOption::DateCreated => t!("files.sort.created"),
            SortOption::Size => t!("files.sort.size"),
            SortOption::FileType => t!("files.sort.type"),
            SortOption::Path => t!("files.sort.path"),
        };
        let arrow = match self.sort_preferences.direction {
            SortDirection::Ascending => "↑",
            SortDirection::Descending => "↓",
        };
        format!("{field} {arrow}")
    }

    fn file_list(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.schedule_list_icon_warm(window, cx);
        match self.view_mode {
            ViewMode::Details => self.details_table(window, cx).into_any_element(),
            ViewMode::List => self.list_view(window, cx).into_any_element(),
            ViewMode::Grid => self.grid_view(window, cx).into_any_element(),
            ViewMode::Cards => self.cards_view(window, cx).into_any_element(),
            ViewMode::Columns => self.columns_view(window, cx).into_any_element(),
        }
    }

    fn columns_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let columns = self
            .column_trail
            .iter()
            .enumerate()
            .zip(self.column_listings.iter())
            .zip(self.column_scroll_handles.iter())
            .map(|(((col_index, col_path), items), scroll_handle)| {
                let title = col_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| col_path.to_string_lossy().to_string());
                let selected_name = self.column_selection_name(col_index);
                let item_count = items.len();
                let item_sizes = Rc::new(vec![COLUMN_ROW_SIZE; item_count.max(1)]);

                let is_active = self.active_column_index == Some(col_index);

                v_flex()
                    .id(("files-column", col_index))
                    .w(COLUMN_WIDTH)
                    .flex_none()
                    .h_full()
                    .min_h_0()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.activate_column(col_index, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        h_flex()
                            .h_8()
                            .px_2()
                            .flex_none()
                            .items_center()
                            .bg(if is_active {
                                cx.theme().primary
                            } else {
                                cx.theme().muted
                            })
                            .text_xs()
                            .text_color(if is_active {
                                cx.theme().primary_foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(title),
                    )
                    .child(
                        v_flex()
                            .id(("files-column-content", col_index))
                            .flex_1()
                            .min_h_0()
                            .size_full()
                            .overflow_hidden()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    this.begin_sweep_selection(
                                        SweepSelectionSurface::Column(col_index),
                                        event.position,
                                        event.modifiers,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }),
                            )
                            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                                this.update_sweep_pointer(
                                    SweepSelectionSurface::Column(col_index),
                                    event.position,
                                    cx,
                                );
                            }))
                            .on_prepaint({
                                let entity = cx.entity().clone();
                                move |bounds, _window, cx| {
                                    let _ = entity.update(cx, |this, _cx| {
                                        this.column_sweep_bounds = Some((col_index, bounds));
                                    });
                                }
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| {
                                    this.finish_sweep_selection();
                                }),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| {
                                    this.finish_sweep_selection();
                                }),
                            )
                            .child(
                                v_virtual_list(
                                    cx.entity().clone(),
                                    format!("files-column-virtual-list-{col_index}"),
                                    item_sizes,
                                    move |this, visible_range, window, cx| {
                                        let has_explicit_column_selection = this
                                            .column_listings
                                            .get(col_index)
                                            .is_some_and(|items| {
                                                items.iter().any(|item| {
                                                    this.selected_paths.contains(&item.path)
                                                })
                                            })
                                            || this
                                                .column_selected_path
                                                .as_ref()
                                                .is_some_and(|(selected_col, _)| {
                                                    *selected_col == col_index
                                                });
                                        visible_range
                                            .filter_map(|index| {
                                                let item = this.column_listings.get(col_index)?.get(index)?.clone();
                                                let is_selected = if has_explicit_column_selection {
                                                    if item.kind == FileItemKind::Folder {
                                                        this.selected_paths.contains(&item.path)
                                                    } else {
                                                        this.column_selected_path
                                                            == Some((col_index, item.path.clone()))
                                                            || this.selected_paths.contains(&item.path)
                                                    }
                                                } else if item.kind == FileItemKind::Folder {
                                                    selected_name.as_deref()
                                                        == Some(item.display_name.as_str())
                                                        || this.selected_paths.contains(&item.path)
                                                } else {
                                                    this.column_selected_path
                                                        == Some((col_index, item.path.clone()))
                                                        || this.selected_paths.contains(&item.path)
                                                };
                                                let drag_paths = vec![item.path.clone()];
                                                Some(Self::column_cell(
                                                    window, col_index, index, item, is_selected, drag_paths, cx,
                                                ))
                                            })
                                            .collect()
                                    },
                                )
                                .track_scroll(scroll_handle),
                            )
                            .when_some(self.render_column_sweep_overlay(col_index, cx), |this, overlay| {
                                this.child(overlay)
                            }),
                    )
                    .scrollbar(scroll_handle, ScrollbarAxis::Vertical)
            })
            .collect::<Vec<_>>();

        h_flex()
            .id("files-columns-wrap")
            .size_full()
            .flex_1()
            .min_h_0()
            .w_full()
            .items_start()
            .overflow_x_scroll()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.active_column_index = None;
                    this.clear_selection();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.clear_selection();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .children(columns)
    }

    fn column_cell(
        window: &mut Window,
        col_index: usize,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let kind = item.kind;
        let name = item.display_name.clone();
        let item_click = item.clone();
        let item_click_path = item_click.path.clone();
        h_flex()
            .id(format!("file-column-row-{col_index}-{name}"))
            .w_full()
            .h_8()
            .flex_none()
            .px_2()
            .gap_2()
            .items_center()
            .text_sm()
            .text_color(cx.theme().foreground)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    if event.modifiers.shift || event.modifiers.secondary() {
                        this.begin_sweep_selection(
                            SweepSelectionSurface::Column(col_index),
                            event.position,
                            event.modifiers,
                            cx,
                        );
                    }
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.focus(&this.focus_handle, cx);
                if event.modifiers().shift || event.modifiers().secondary() {
                    this.handle_column_item_click(
                        col_index,
                        index,
                        &item_click,
                        event.modifiers(),
                        cx,
                    );
                } else if kind == FileItemKind::Folder {
                    this.select_column_item(col_index, &item_click, cx);
                } else if event.click_count() == 2 {
                    this.open_item(item_click.path.clone(), kind, cx);
                } else {
                    this.handle_column_item_click(
                        col_index,
                        index,
                        &item_click,
                        event.modifiers(),
                        cx,
                    );
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_column_context_menu_target(col_index, index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(
                    SweepSelectionSurface::Column(col_index),
                    index,
                    cx,
                );
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                |paths, _offset, _window, cx| {
                    let label = if paths.0.len() == 1 {
                        paths.0[0]
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| t!("files.type.file").to_string())
                    } else {
                        format!("{} {}", paths.0.len(), t!("files.status.items"))
                    };
                    cx.new(|_| DragPathPreview {
                        label: label.into(),
                    })
                },
            )
            .child(
                div()
                    .w(px(20.))
                    .flex_none()
                    .child(Self::row_list_icon(&item, px(16.), window)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .child(name),
            )
            .into_any_element()
    }

    fn details_table(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("files-details-table")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .child(
                h_flex()
                    .h_8()
                    .px_3()
                    .gap_3()
                    .items_center()
                    .bg(cx.theme().muted)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().w(px(28.)).flex_none())
                    .child(div().flex_1().min_w_0().child(t!("files.column.name")))
                    .child(div().w(px(110.)).child(t!("files.column.type")))
                    .child(div().w(px(100.)).child(t!("files.column.size")))
                    .child(div().w(px(150.)).child(t!("files.column.modified")))
                    .child(div().w(px(40.)).flex_none()),
            )
            .child(
                v_flex()
                    .id("files-virtual-list-wrap")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-virtual-list",
                            self.item_sizes.clone(),
                            |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|index| {
                                        let item = this.display_items.get(index)?.clone();
                                        let selected = this.selected_paths.contains(&item.path);
                                        let drag_paths =
                                            this.drag_paths_for_item(index, &item.path);
                                        Some(Self::row(
                                            window, index, item, selected, drag_paths, cx,
                                        ))
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    )
                    .scrollbar(&self.scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn list_view(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("files-list-view")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.clear_selection();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .child(
                h_flex()
                    .h_8()
                    .px_3()
                    .gap_3()
                    .items_center()
                    .bg(cx.theme().muted)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().w(px(28.)).flex_none())
                    .child(div().flex_1().min_w_0().child(t!("files.column.name")))
                    .child(div().w(px(40.)).flex_none()),
            )
            .child(
                v_flex()
                    .id("files-virtual-list-wrap")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-virtual-list",
                            self.item_sizes.clone(),
                            |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|index| {
                                        let item = this.display_items.get(index)?.clone();
                                        let selected = this.selected_paths.contains(&item.path);
                                        let drag_paths =
                                            this.drag_paths_for_item(index, &item.path);
                                        Some(Self::list_row(
                                            window, index, item, selected, drag_paths, cx,
                                        ))
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    )
                    .scrollbar(&self.scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn list_row(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open_path = item.path.clone();
        let double_click_path = item.path.clone();
        let kind = item.kind;
        h_flex()
            .id(("file-list-row", index))
            .w_full()
            .h_9()
            .flex_none()
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground)
                    .child(Self::row_list_icon(&item, px(16.), window)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(item.display_name.clone()),
            )
            .into_any_element()
    }

    fn grid_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (cell_w, cell_h, icon_size) = match self.view_size_level {
            1 => (px(96.), px(72.), px(18.)),
            3 => (px(144.), px(104.), px(26.)),
            _ => (px(112.), px(80.), px(22.)),
        };

        // Estimate available width from the viewport; fall back to this when we
        // have no measured cells-per-row yet.
        let estimated_available_width = {
            let sidebar_w = px(240.);
            let info_pane_w = if self.show_info_pane { px(300.) } else { px(0.) };
            let padding_border = px(18.); // grid-view border(2) + grid-wrap padding(16)
            (window.viewport_size().width - sidebar_w - info_pane_w - padding_border).max(px(200.))
        };
        let gap = px(8.);
        let estimated_cells_per_row =
            ((estimated_available_width + gap) / (cell_w + gap)).max(1.) as usize;
        let cells_per_row = self.grid_cells_per_row.unwrap_or(estimated_cells_per_row);
        let row_count = (self.display_items.len() + cells_per_row.saturating_sub(1)) / cells_per_row;
        let item_sizes = Rc::new(vec![size(px(1.), cell_h); row_count.max(1)]);

        v_flex()
            .id("files-grid-view")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.clear_selection();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_prepaint({
                let entity = cx.entity().clone();
                move |bounds, window, cx| {
                    let measured_width = bounds.size.width - px(18.); // subtract border(2)+padding(16)
                    let measured_cells =
                        ((measured_width + gap) / (cell_w + gap)).max(1.) as usize;
                    let changed = entity.update(cx, |this, _cx| {
                        let changed = this.grid_cells_per_row != Some(measured_cells);
                        this.grid_cells_per_row = Some(measured_cells);
                        changed
                    });
                    if changed {
                        window.refresh();
                    }
                }
            })
            .child(
                v_flex()
                    .id("files-grid-wrap")
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .p_2()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-grid-virtual-list",
                            item_sizes,
                            move |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|row_ix| {
                                        let start = row_ix * cells_per_row;
                                        let end = (start + cells_per_row).min(this.display_items.len());
                                        if start >= this.display_items.len() {
                                            return None;
                                        }
                                        Some(
                                            h_flex()
                                                .w_full()
                                                .gap_2()
                                                .children(
                                                    (start..end).map(|index| {
                                                        let item = this.display_items[index].clone();
                                                        let selected = this.selected_paths.contains(&item.path);
                                                        let drag_paths = this.drag_paths_for_item(index, &item.path);
                                                        Self::grid_cell(window, index, item, selected, drag_paths, cell_w, cell_h, icon_size, cx)
                                                    })
                                                )
                                                .into_any_element(),
                                        )
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.grid_scroll_handle)
                        .gap_2(),
                    )
                    .scrollbar(&self.grid_scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn cards_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cell_w = px(160.);
        let cell_h = px(120.);

        // Estimate available width from the viewport; fall back to this when we
        // have no measured cells-per-row yet.
        let estimated_available_width = {
            let sidebar_w = px(240.);
            let info_pane_w = if self.show_info_pane { px(300.) } else { px(0.) };
            let padding_border = px(18.); // cards-view border(2) + cards-wrap padding(16)
            (window.viewport_size().width - sidebar_w - info_pane_w - padding_border).max(px(200.))
        };
        let gap = px(8.);
        let estimated_cells_per_row =
            ((estimated_available_width + gap) / (cell_w + gap)).max(1.) as usize;
        let cells_per_row = self.cards_cells_per_row.unwrap_or(estimated_cells_per_row);
        let row_count = (self.display_items.len() + cells_per_row.saturating_sub(1)) / cells_per_row;
        let item_sizes = Rc::new(vec![size(px(1.), cell_h); row_count.max(1)]);

        v_flex()
            .id("files-cards-view")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.clear_selection();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_prepaint({
                let entity = cx.entity().clone();
                move |bounds, window, cx| {
                    let measured_width = bounds.size.width - px(18.); // subtract border(2)+padding(16)
                    let measured_cells =
                        ((measured_width + gap) / (cell_w + gap)).max(1.) as usize;
                    let changed = entity.update(cx, |this, _cx| {
                        let changed = this.cards_cells_per_row != Some(measured_cells);
                        this.cards_cells_per_row = Some(measured_cells);
                        changed
                    });
                    if changed {
                        window.refresh();
                    }
                }
            })
            .child(
                v_flex()
                    .id("files-cards-wrap")
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .p_2()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-cards-virtual-list",
                            item_sizes,
                            move |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|row_ix| {
                                        let start = row_ix * cells_per_row;
                                        let end = (start + cells_per_row).min(this.display_items.len());
                                        if start >= this.display_items.len() {
                                            return None;
                                        }
                                        Some(
                                            h_flex()
                                                .w_full()
                                                .gap_2()
                                                .children(
                                                    (start..end).map(|index| {
                                                        let item = this.display_items[index].clone();
                                                        let selected = this.selected_paths.contains(&item.path);
                                                        let drag_paths = this.drag_paths_for_item(index, &item.path);
                                                        Self::card_cell(window, index, item, selected, drag_paths, cx)
                                                    })
                                                )
                                                .into_any_element(),
                                        )
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.cards_scroll_handle)
                        .gap_2(),
                    )
                    .scrollbar(&self.cards_scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn row(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open_path = item.path.clone();
        let double_click_path = item.path.clone();
        let kind = item.kind;
        h_flex()
            .id(("file-row", index))
            .w_full()
            .h_9()
            .flex_none()
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground)
                    .child(Self::row_list_icon(&item, px(16.), window)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(item.display_name.clone()),
            )
            .child(
                div()
                    .w(px(110.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(item_type_label(&item)),
            )
            .child(
                div()
                    .w(px(100.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_size(item.size)),
            )
            .child(
                div()
                    .w(px(150.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_system_time(item.modified)),
            )
            .child(
                div().w(px(40.)).flex_none().child(
                    toolbar_icon_button(format!("open-item-{index}"))
                        .icon(match kind {
                            FileItemKind::Folder => compact_icon(IconName::ChevronRight),
                            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                                compact_icon(IconName::ExternalLink)
                            }
                        })
                        .tooltip(match kind {
                            FileItemKind::Folder => t!("files.open.folder"),
                            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                                t!("files.open.file")
                            }
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_item(open_path.clone(), kind, cx);
                        })),
                ),
            )
            .into_any_element()
    }

    fn grid_cell(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        cell_w: Pixels,
        cell_h: Pixels,
        icon_size: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open_path = item.path.clone();
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let name = item.display_name.clone();
        v_flex()
            .id(("file-grid-cell", index))
            .w(cell_w)
            .h(cell_h)
            .flex_none()
            .p_2()
            .gap_1()
            .items_center()
            .justify_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .child(Self::row_list_icon(&item, icon_size, window))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_xs()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
            .into_any_element()
    }

    fn card_cell(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open_path = item.path.clone();
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let name = item.display_name.clone();
        v_flex()
            .id(("file-card-cell", index))
            .w(px(160.))
            .h(px(120.))
            .flex_none()
            .p_2()
            .gap_1()
            .items_center()
            .justify_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .child(Self::row_list_icon(&item, px(40.), window))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_sm()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
            .when(item.size.is_some(), |this| {
                this.child(
                    div()
                        .w_full()
                        .text_center()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format_size(item.size)),
                )
            })
            .when(item.modified.is_some(), |this| {
                this.child(
                    div()
                        .w_full()
                        .text_center()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format_system_time(item.modified)),
                )
            })
            .into_any_element()
    }

    fn rename_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let renaming = self.renaming.as_ref()?;
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!("files.rename.prompt")),
                )
                .child(div().flex_1().child(Input::new(&renaming.input)))
                .child(
                    Button::new("rename-confirm")
                        .small()
                        .primary()
                        .label(t!("files.rename.confirm"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.commit_rename(window, cx);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("rename-cancel")
                        .small()
                        .ghost()
                        .label(t!("files.cancel"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cancel_rename();
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}

impl Focusable for FileBrowser {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl FileBrowser {
    fn on_refresh(&mut self, _: &RefreshDirectory, _: &mut Window, cx: &mut Context<Self>) {
        self.refresh();
        cx.notify();
    }

    fn on_open_item(&mut self, _: &OpenItem, _: &mut Window, cx: &mut Context<Self>) {
        self.open_focused(cx);
    }

    fn on_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all();
        cx.stop_propagation();
        cx.notify();
    }

    fn on_rename(&mut self, _: &RenameItem, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_rename(window, cx);
        cx.notify();
    }

    fn on_delete(&mut self, _: &DeleteItems, window: &mut Window, cx: &mut Context<Self>) {
        self.perform_delete(window, cx);
        cx.notify();
    }

    fn on_delete_permanent(
        &mut self,
        _: &DeleteItemsPermanent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.perform_delete_permanent(window, cx);
        cx.notify();
    }

    fn on_copy_items(&mut self, _: &CopyItems, _: &mut Window, cx: &mut Context<Self>) {
        self.copy_items(cx);
        cx.notify();
    }

    fn on_cut_items(&mut self, _: &CutItems, _: &mut Window, cx: &mut Context<Self>) {
        self.cut_items(cx);
        cx.notify();
    }

    fn on_compress_items(
        &mut self,
        _: &CompressItems,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.compress_items(window, cx);
    }

    fn on_paste_items(&mut self, _: &PasteItems, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_items(window, cx);
    }

    fn on_new_folder(&mut self, _: &NewFolder, window: &mut Window, cx: &mut Context<Self>) {
        self.create_new_folder(window, cx);
        cx.notify();
    }

    fn on_copy_path(&mut self, _: &CopyPath, _: &mut Window, cx: &mut Context<Self>) {
        self.copy_paths(cx);
    }

    fn on_navigate_previous(
        &mut self,
        _: &NavigatePrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_focus(-1);
        cx.notify();
    }

    fn on_navigate_next(&mut self, _: &NavigateNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_focus(1);
        cx.notify();
    }

    fn on_sort_name(&mut self, _: &SortByName, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::Name);
        cx.notify();
    }

    fn on_sort_modified(&mut self, _: &SortByModified, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::DateModified);
        cx.notify();
    }

    fn on_sort_created(&mut self, _: &SortByCreated, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::DateCreated);
        cx.notify();
    }

    fn on_sort_size(&mut self, _: &SortBySize, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::Size);
        cx.notify();
    }

    fn on_sort_type(&mut self, _: &SortByType, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::FileType);
        cx.notify();
    }

    fn on_toggle_sort_direction(
        &mut self,
        _: &ToggleSortDirection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sort_preferences.direction = match self.sort_preferences.direction {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        };
        self.refresh();
        self.persist_prefs();
        cx.notify();
    }

    fn on_toggle_show_hidden(
        &mut self,
        _: &ToggleShowHidden,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.read_options.show_hidden_items = !self.read_options.show_hidden_items;
        self.read_options.show_dot_files = self.read_options.show_hidden_items;
        self.refresh();
        self.persist_prefs();
        cx.notify();
    }

    fn on_toggle_show_file_extensions(
        &mut self,
        _: &ToggleShowFileExtensions,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.read_options.show_file_extensions = !self.read_options.show_file_extensions;
        self.refresh();
        self.persist_prefs();
        cx.notify();
    }

    fn on_open_in_new_pane(&mut self, _: &OpenInNewPane, _: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.primary_path() else {
            return;
        };
        AppNavigation::open_path_in_secondary_pane(path, cx);
    }

    fn on_open_in_terminal(
        &mut self,
        _: &OpenInTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut paths: Vec<PathBuf> = self
            .selected_paths_vec()
            .into_iter()
            .filter(|path| path.is_dir())
            .collect();
        if paths.is_empty() {
            let Some(path) = self.primary_path() else {
                return;
            };
            paths.push(path);
        }
        if let Err(error) = open_paths_in_terminal(&paths) {
            window.push_notification(
                Notification::error(format!("{}: {error}", t!("files.terminal.error"))),
                cx,
            );
        }
    }

    fn on_create_folder_from_selection(
        &mut self,
        _: &CreateFolderFromSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.create_folder_from_selection(window, cx);
    }

    fn on_open_in_new_window(
        &mut self,
        _: &OpenInNewWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.primary_path() else {
            return;
        };
        if let Err(error) = platform::open_in_new_explorer_window(&path) {
            window.push_notification(
                Notification::error(format!("{}: {error}", t!("files.open_new_window.error"))),
                cx,
            );
        }
    }

    fn on_open_with_dialog(
        &mut self,
        _: &OpenWithDialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.primary_path() else {
            return;
        };
        if path.is_dir() {
            return;
        }
        if let Err(error) = platform::show_open_with_dialog(&path) {
            window.push_notification(
                Notification::error(format!("{}: {error}", t!("files.open_with.error"))),
                cx,
            );
        }
    }

    fn on_create_shortcut(
        &mut self,
        _: &CreateShortcut,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        if let Err(error) = create_shortcuts_for_paths(&paths) {
            window.push_notification(
                Notification::error(format!("{}: {error}", t!("files.create_shortcut.error"))),
                cx,
            );
        } else {
            window.push_notification(
                Notification::success(t!("files.create_shortcut.success")),
                cx,
            );
            self.refresh();
            cx.notify();
        }
    }

    fn on_new_file(&mut self, _: &NewFile, window: &mut Window, cx: &mut Context<Self>) {
        self.create_new_file(window, cx);
        cx.notify();
    }

    fn on_view_details(&mut self, _: &ViewDetails, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Details, cx);
    }

    fn on_view_grid(&mut self, _: &ViewGrid, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Grid, cx);
    }

    fn on_view_cards(&mut self, _: &ViewCards, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Cards, cx);
    }

    fn on_view_list(&mut self, _: &ViewList, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::List, cx);
    }

    fn on_view_columns(&mut self, _: &ViewColumns, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Columns, cx);
    }

    fn on_focus_search_action(
        &mut self,
        _: &FocusSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_search(window, cx);
    }

    fn on_shell_properties(
        &mut self,
        _: &ShellProperties,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_properties(cx);
    }

    fn show_properties(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.primary_path() else {
            return;
        };
        let path = path.to_path_buf();
        cx.spawn(async move |_, cx| {
            let _ = cx
                .background_spawn(async move { platform::open_item_properties(&path) })
                .await;
        })
        .detach();
    }
}

impl FileBrowser {
    /// Files-style toolbar above the file list (view, sort, new, delete).
    fn render_content_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_count = self.selected_paths.len();
        let show_hidden = self.read_options.show_hidden_items;
        let show_file_extensions = self.read_options.show_file_extensions;
        let sort_label = self.sort_label();

        let can_paste = AppFileClipboard::has_items(cx);

        h_flex()
            .id("content-toolbar")
            .w_full()
            .flex_none()
            .gap_2()
            .px_3()
            .py_1()
            .mb_1()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            // Context Commands
            .child(
                toolbar_icon_button("content-cut")
                    .icon(toolbar_icon(IconName::Replace).path("icons/content_cut.svg"))
                    .tooltip(t!("files.menu.cut"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.cut_items(cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-copy")
                    .icon(toolbar_icon(IconName::Copy).path("icons/content_copy.svg"))
                    .tooltip(t!("files.menu.copy"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.copy_items(cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-paste")
                    .icon(toolbar_icon(IconName::Replace).path("icons/content_paste.svg"))
                    .tooltip(t!("files.menu.paste"))
                    .disabled(!can_paste)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.paste_items(window, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-rename")
                    .icon(toolbar_icon(IconName::File).path("icons/drive_file_rename_outline.svg"))
                    .tooltip(t!("files.menu.rename"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.begin_rename(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-properties")
                    .icon(toolbar_icon(IconName::Info))
                    .tooltip(t!("files.menu.properties"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_properties(cx);
                    })),
            )
            .child(div().w(px(1.)).h(px(20.)).bg(cx.theme().border))
            .child(
                toolbar_icon_button("content-new-folder")
                    .size(TOOLBAR_BUTTON_PX)
                    .icon(toolbar_icon(IconName::Folder).path("icons/create_new_folder.svg"))
                    .tooltip(t!("files.new_folder"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.create_new_folder(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-new-file")
                    .size(TOOLBAR_BUTTON_PX)
                    .icon(toolbar_icon(IconName::File).path("icons/note_add.svg"))
                    .tooltip(t!("files.new_file"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.create_new_file(window, cx);
                        cx.notify();
                    })),
            )
            .child(div().w(px(1.)).h(px(20.)).bg(cx.theme().border))
            .child(
                toolbar_icon_button("content-view-details")
                    .icon(toolbar_icon(IconName::GalleryVerticalEnd).path("icons/view_headline.svg"))
                    .tooltip(t!("files.view.details"))
                    .when(self.view_mode == ViewMode::Details, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Details, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-list")
                    .icon(toolbar_icon(IconName::PanelLeftOpen))
                    .tooltip(t!("files.view.list"))
                    .when(self.view_mode == ViewMode::List, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::List, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-grid")
                    .icon(toolbar_icon(IconName::LayoutDashboard))
                    .tooltip(t!("files.view.grid"))
                    .when(self.view_mode == ViewMode::Grid, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Grid, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-cards")
                    .icon(toolbar_icon(IconName::LayoutDashboard).path("icons/view_cozy.svg"))
                    .tooltip(t!("files.view.cards"))
                    .when(self.view_mode == ViewMode::Cards, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Cards, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-columns")
                    .icon(toolbar_icon(IconName::PanelLeft))
                    .tooltip(t!("files.view.columns"))
                    .when(self.view_mode == ViewMode::Columns, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Columns, cx);
                    })),
            )
            .child(div().w(px(1.)).h(px(20.)).bg(cx.theme().border))
            .child(
                toolbar_icon_button("content-delete")
                    .icon(toolbar_icon(IconName::Delete))
                    .tooltip(t!("files.menu.delete"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.perform_delete(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_dropdown_button("content-sort")
                    .button(
                        toolbar_labeled_button("content-sort-btn")
                            .label(sort_label)
                            .tooltip(t!("files.menu.sort")),
                    )
                    .dropdown_menu(move |menu, _, _| {
                        let hidden_label = if show_hidden {
                            t!("files.show_hidden.off")
                        } else {
                            t!("files.show_hidden.on")
                        };
                        let extensions_label = if show_file_extensions {
                            t!("files.show_extensions.off")
                        } else {
                            t!("files.show_extensions.on")
                        };
                        menu.menu(t!("files.sort.name"), Box::new(SortByName))
                            .menu(t!("files.sort.modified"), Box::new(SortByModified))
                            .menu(t!("files.sort.size"), Box::new(SortBySize))
                            .menu(t!("files.sort.type"), Box::new(SortByType))
                            .separator()
                            .menu(
                                t!("files.sort.toggle_direction"),
                                Box::new(ToggleSortDirection),
                            )
                            .menu(hidden_label, Box::new(ToggleShowHidden))
                            .menu(extensions_label, Box::new(ToggleShowFileExtensions))
                    }),
            )
    }
}

impl Render for FileBrowser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.watched_dir.as_ref() != Some(&self.current_dir) {
            self.watched_dir = Some(self.current_dir.clone());
            self.restart_directory_watcher(cx);
        }

        // Invalidate caches when the window is resized so we don't render with
        // stale measurements from a previous size.
        let viewport_width = window.viewport_size().width;
        if self.last_viewport_width != Some(viewport_width) {
            self.last_viewport_width = Some(viewport_width);
            self.grid_cells_per_row = None;
            self.cards_cells_per_row = None;
        }

        let current_dir = self.current_dir.to_string_lossy().to_string();
        let can_go_back = !self.back_stack.is_empty();
        let can_go_forward = !self.forward_stack.is_empty();
        let can_go_up = self.current_dir.parent().is_some();
        let selected_count = self.selected_paths.len();
        let show_hidden = self.read_options.show_hidden_items;
        let show_file_extensions = self.read_options.show_file_extensions;
        let sort_label = self.sort_label();
        let in_recycle_bin = self.browse_location == BrowseLocation::RecycleBin;

        let page_gap = if self.show_content_toolbar && !self.show_toolbar {
            px(0.)
        } else {
            px(12.)
        };

        v_flex()
            .id("files-page")
            .size_full()
            .min_h_0()
            .gap(page_gap)
            .track_focus(&self.focus_handle)
            .key_context(FILE_BROWSER)
            .on_action(cx.listener(Self::on_refresh))
            .on_action(cx.listener(Self::on_open_item))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_rename))
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(Self::on_delete_permanent))
            .on_action(cx.listener(Self::on_new_folder))
            .on_action(cx.listener(Self::on_new_file))
            .on_action(cx.listener(Self::on_view_details))
            .on_action(cx.listener(Self::on_view_list))
            .on_action(cx.listener(Self::on_view_grid))
            .on_action(cx.listener(Self::on_view_cards))
            .on_action(cx.listener(Self::on_view_columns))
            .on_action(cx.listener(Self::on_focus_search_action))
            .on_action(cx.listener(Self::on_shell_properties))
            .on_drop(cx.listener(|this, paths: &DraggedFilePaths, window, cx| {
                this.handle_drop(paths.0.clone(), window, cx);
            }))
            .on_action(cx.listener(Self::on_copy_path))
            .on_action(cx.listener(Self::on_copy_items))
            .on_action(cx.listener(Self::on_cut_items))
            .on_action(cx.listener(Self::on_paste_items))
            .on_action(cx.listener(Self::on_compress_items))
            .on_action(cx.listener(Self::on_navigate_previous))
            .on_action(cx.listener(Self::on_navigate_next))
            .on_action(cx.listener(Self::on_sort_name))
            .on_action(cx.listener(Self::on_sort_created))
            .on_action(cx.listener(Self::on_sort_modified))
            .on_action(cx.listener(Self::on_sort_size))
            .on_action(cx.listener(Self::on_sort_type))
            .on_action(cx.listener(Self::on_toggle_sort_direction))
            .on_action(cx.listener(Self::on_toggle_show_hidden))
            .on_action(cx.listener(Self::on_toggle_show_file_extensions))
            .on_action(cx.listener(Self::on_open_in_new_pane))
            .on_action(cx.listener(Self::on_open_in_terminal))
            .on_action(cx.listener(Self::on_create_folder_from_selection))
            .on_action(cx.listener(Self::on_open_in_new_window))
            .on_action(cx.listener(Self::on_open_with_dialog))
            .on_action(cx.listener(Self::on_create_shortcut))
            .when(self.show_content_toolbar, |this| {
                this.child(self.render_content_toolbar(cx))
            })
            .when(self.show_toolbar, |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .flex_wrap()
                        .child(
                            toolbar_icon_button("files-back")
                                .icon(toolbar_icon(IconName::ArrowLeft))
                                .tooltip(t!("nav.back"))
                                .disabled(!can_go_back)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_back(cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-forward")
                                .icon(toolbar_icon(IconName::ArrowRight))
                                .tooltip(t!("nav.forward"))
                                .disabled(!can_go_forward)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_forward(cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-up")
                                .icon(toolbar_icon(IconName::ArrowUp))
                                .tooltip(t!("nav.up"))
                                .disabled(!can_go_up)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_parent(cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-refresh")
                                .icon(toolbar_icon(IconName::Redo2))
                                .tooltip(t!("nav.refresh"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh();
                                    cx.notify();
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-new-folder-btn")
                                .size(TOOLBAR_BUTTON_PX)
                                .icon(toolbar_icon(IconName::Folder).path("icons/create_new_folder.svg"))
                                .tooltip(t!("files.new_folder"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.create_new_folder(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-new-file-btn")
                                .size(TOOLBAR_BUTTON_PX)
                                .icon(toolbar_icon(IconName::File).path("icons/note_add.svg"))
                                .tooltip(t!("files.new_file"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.create_new_file(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-details")
                                .icon(toolbar_icon(IconName::GalleryVerticalEnd).path("icons/view_headline.svg"))
                                .tooltip(t!("files.view.details"))
                                .when(self.view_mode == ViewMode::Details, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_view_mode(ViewMode::Details, cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-list")
                                .icon(toolbar_icon(IconName::PanelLeftOpen))
                                .tooltip(t!("files.view.list"))
                                .when(self.view_mode == ViewMode::List, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_view_mode(ViewMode::List, cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-grid")
                                .icon(toolbar_icon(IconName::LayoutDashboard))
                                .tooltip(t!("files.view.grid"))
                                .when(self.view_mode == ViewMode::Grid, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_view_mode(ViewMode::Grid, cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-cards")
                                .icon(toolbar_icon(IconName::LayoutDashboard).path("icons/view_cozy.svg"))
                                .tooltip(t!("files.view.cards"))
                                .when(self.view_mode == ViewMode::Cards, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_view_mode(ViewMode::Cards, cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-columns")
                                .icon(toolbar_icon(IconName::PanelLeft))
                                .tooltip(t!("files.view.columns"))
                                .when(self.view_mode == ViewMode::Columns, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_view_mode(ViewMode::Columns, cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-delete-btn")
                                .icon(toolbar_icon(IconName::Delete))
                                .tooltip(t!("files.menu.delete"))
                                .disabled(selected_count == 0)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.perform_delete(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            toolbar_dropdown_button("files-sort")
                                .button(
                                    toolbar_labeled_button("files-sort-btn")
                                        .label(sort_label)
                                        .tooltip(t!("files.menu.sort")),
                                )
                                .dropdown_menu(move |menu, _, _| {
                                    let hidden_label = if show_hidden {
                                        t!("files.show_hidden.off")
                                    } else {
                                        t!("files.show_hidden.on")
                                    };
                                    let extensions_label = if show_file_extensions {
                                        t!("files.show_extensions.off")
                                    } else {
                                        t!("files.show_extensions.on")
                                    };
                                    menu.menu(t!("files.sort.name"), Box::new(SortByName))
                                        .menu(t!("files.sort.modified"), Box::new(SortByModified))
                                        .menu(t!("files.sort.size"), Box::new(SortBySize))
                                        .menu(t!("files.sort.type"), Box::new(SortByType))
                                        .separator()
                                        .menu(
                                            t!("files.sort.toggle_direction"),
                                            Box::new(ToggleSortDirection),
                                        )
                                        .menu(hidden_label, Box::new(ToggleShowHidden))
                                        .menu(extensions_label, Box::new(ToggleShowFileExtensions))
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(120.))
                                .px_3()
                                .py_1()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .text_color(cx.theme().muted_foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(current_dir),
                        ),
                )
            })
            .when_some(self.rename_bar(window, cx), |this, bar| this.child(bar))
            .when_some(self.error.as_ref(), |this, error| {
                this.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().danger)
                        .text_color(cx.theme().danger)
                        .child(error.clone()),
                )
            })
            .when(in_recycle_bin, |this| {
                this.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().muted)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!("files.recycle.hint")),
                )
            })
            .child(
                div()
                    .id("files-list-host")
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.begin_sweep_selection(
                                SweepSelectionSurface::Main,
                                event.position,
                                event.modifiers,
                                cx,
                            );
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.update_sweep_pointer(
                            SweepSelectionSurface::Main,
                            event.position,
                            cx,
                        );
                    }))
                    .on_prepaint({
                        let entity = cx.entity().clone();
                        move |bounds, _window, cx| {
                            let _ = entity.update(cx, |this, _cx| {
                                this.main_sweep_bounds = Some(bounds);
                            });
                        }
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| {
                            this.finish_sweep_selection();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| {
                            this.finish_sweep_selection();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(|this, _, _, cx| {
                            this.clear_selection();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.clear_selection();
                            this.set_context_menu_extended_verbs(event.modifiers.shift);
                            this.open_context_menu(event.position, window, cx);
                        }),
                    )
                    .child(self.file_list(window, cx))
                    .when_some(self.render_main_sweep_overlay(cx), |this, overlay| {
                        this.child(overlay)
                    }),
            )
            .when(self.context_menu_open, |this| {
                this.child(self.render_context_menu_overlay(window))
            })
    }
}

fn paths_for_file_tag(tag_name: &str) -> Vec<PathBuf> {
    let config = load_config().unwrap_or_default();
    config
        .file_tags
        .iter()
        .find(|tag| tag.name == tag_name)
        .map(|tag| {
            tag.paths
                .iter()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}

fn load_files_dir(
    path: &Path,
    options: DirectoryReadOptions,
    sort: SortPreferences,
) -> (Vec<FileItem>, Option<String>) {
    match read_directory(path, options, sort) {
        Ok(items) => (items, None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    }
}

fn item_sizes_for(count: usize, mode: ViewMode, size_level: u8) -> Rc<Vec<Size<Pixels>>> {
    let size = match mode {
        ViewMode::Details | ViewMode::List => match size_level {
            1 => FILE_ROW_SIZE_COMPACT,
            3 => FILE_ROW_SIZE_LARGE,
            _ => FILE_ROW_SIZE,
        },
        ViewMode::Grid => match size_level {
            1 => GRID_CELL_SIZE_SMALL,
            3 => GRID_CELL_SIZE_LARGE,
            _ => GRID_CELL_SIZE,
        },
        ViewMode::Cards => CARD_CELL_SIZE,
        ViewMode::Columns => COLUMN_ROW_SIZE,
    };
    Rc::new(vec![size; count.max(1)])
}

fn column_listings_for(
    trail: &[PathBuf],
    read_options: &DirectoryReadOptions,
    sort: SortPreferences,
    query: &str,
) -> Vec<Vec<FileItem>> {
    trail
        .iter()
        .map(|path| {
            let (items, _) = load_files_dir(path, *read_options, sort);
            filter_items_by_query(&items, query)
        })
        .collect()
}

fn drag_preview_label(paths: &[PathBuf]) -> String {
    if paths.len() == 1 {
        paths[0]
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| t!("files.type.file").to_string())
    } else {
        format!("{} {}", paths.len(), t!("files.status.items"))
    }
}

fn clamp_point_to_bounds(position: Point<Pixels>, bounds: Bounds<Pixels>) -> Point<Pixels> {
    point(
        position.x.max(bounds.origin.x).min(bounds.right()),
        position.y.max(bounds.origin.y).min(bounds.bottom()),
    )
}

fn rects_intersect(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    a.left() < b.right() && a.right() > b.left() && a.top() < b.bottom() && a.bottom() > b.top()
}

fn sort_option_from_config(value: &str) -> SortOption {
    match value {
        "modified" => SortOption::DateModified,
        "created" => SortOption::DateCreated,
        "size" => SortOption::Size,
        "type" => SortOption::FileType,
        "path" => SortOption::Path,
        _ => SortOption::Name,
    }
}

fn sort_direction_from_config(value: &str) -> SortDirection {
    match value {
        "desc" => SortDirection::Descending,
        _ => SortDirection::Ascending,
    }
}

fn sort_option_config_value(option: SortOption) -> &'static str {
    match option {
        SortOption::Name => "name",
        SortOption::DateModified => "modified",
        SortOption::DateCreated => "created",
        SortOption::Size => "size",
        SortOption::FileType => "type",
        SortOption::Path => "path",
    }
}

#[cfg(windows)]
fn open_paths_in_terminal(paths: &[PathBuf]) -> anyhow::Result<()> {
    use std::path::Path;
    use std::process::Command;

    let dirs = paths
        .iter()
        .map(|path| {
            if path.is_dir() {
                Ok(path.clone())
            } else {
                path.parent()
                    .map(Path::to_path_buf)
                    .ok_or_else(|| anyhow::anyhow!("no parent directory"))
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    if dirs.is_empty() {
        return Ok(());
    }

    let mut args = Vec::with_capacity(dirs.len() * 3);
    for (index, dir) in dirs.iter().enumerate() {
        let dir = dir.to_string_lossy().to_string();
        if index > 0 {
            args.push(";".to_string());
            args.push("nt".to_string());
        }
        args.push("-d".to_string());
        args.push(dir);
    }

    let wt = Command::new("wt.exe").args(&args).spawn();
    if wt.is_ok() {
        return Ok(());
    }

    let dir = dirs[0].to_string_lossy();
    Command::new("cmd")
        .args(["/C", "start", "", "wt.exe", "-d", &dir])
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn open_paths_in_terminal(_paths: &[PathBuf]) -> anyhow::Result<()> {
    anyhow::bail!("terminal launch is only supported on Windows")
}

/// Creates `Shortcut to <name>.lnk` in the parent directory via WScript.Shell.
fn create_shortcut_for_path(path: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent directory"))?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Shortcut".into());
    let link_path = parent.join(format!("Shortcut to {stem}.lnk"));
    let target = path.to_string_lossy().replace('\'', "''");
    let link = link_path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{link}'); $s.TargetPath='{target}'; $s.Save()"
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("powershell shortcut creation failed")
    }
}

fn create_shortcuts_for_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    for path in paths {
        create_shortcut_for_path(path)?;
    }
    Ok(())
}

fn sort_direction_config_value(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "asc",
        SortDirection::Descending => "desc",
    }
}

fn item_type_label(item: &FileItem) -> String {
    match item.kind {
        FileItemKind::Folder => t!("files.type.folder").to_string(),
        FileItemKind::Symlink => t!("files.type.symlink").to_string(),
        FileItemKind::Other => t!("files.type.other").to_string(),
        FileItemKind::File => item
            .extension
            .as_ref()
            .map(|extension| format!("{} file", extension.to_uppercase()))
            .unwrap_or_else(|| t!("files.type.file").to_string()),
    }
}

fn format_size(size: Option<u64>) -> String {
    let Some(size) = size else {
        return String::new();
    };

    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;

    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", size, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_system_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return String::new();
    };

    let local_time: DateTime<Local> = time.into();
    local_time.format("%Y-%m-%d %H:%M").to_string()
}

fn open_with_system(path: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }
}
