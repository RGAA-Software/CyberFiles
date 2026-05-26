use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
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
    CancelRename, CompressItems, CopyItems, CopyPath, CutItems, DeleteItems,
    DeleteItemsPermanent, FocusSearch, NavigateNext, NavigatePrevious, NewFile, NewFolder,
    OpenItem, PasteItems, RefreshDirectory, RenameItem, SelectAll, ShellProperties, ViewCards,
    ViewColumns, ViewDetails, ViewGrid, ViewList, FILE_BROWSER,
};
use cyberfiles_core::{
    file_sort_prefs_from_config, file_view_mode_from_config, load_config, save_file_browser_prefs,
    VIEW_CARDS, VIEW_COLUMNS, VIEW_DETAILS, VIEW_GRID, VIEW_LIST,
};
use cyberfiles_fs::{
    column_trail_for, create_directory, create_file, delete_paths, file_items_for_tag_paths,
    filter_items_by_query, home_navigation_path, move_items, read_directory, read_recycle_bin,
    recycle_paths, rename_path, temp_zip_output_path, unique_new_file_name,
    unique_zip_output_path,
    unique_new_folder_name, ClipboardOperation, DirectoryReadOptions, DirectoryWatcher,
    FileClipboard, FileItem, FileItemKind, SortDirection, SortOption, SortPreferences,
};
use cyberfiles_platform_windows::{self as platform, ShellContextMenuEntry};
use gpui::{
    actions, anchored, deferred, prelude::*, ClickEvent, ClipboardItem, DismissEvent, Entity,
    FocusHandle, Focusable, ParentElement, ScrollStrategy, Subscription, Window, *,
};
use gpui_component::{
    dialog::DialogButtonProps,
    h_flex,
    input::{Input, InputEvent, InputState, SelectAll as InputSelectAll},
    notification::Notification,
    scroll::{ScrollableElement as _, Scrollbar, ScrollbarAxis, ScrollbarShow},
    v_flex, v_virtual_list, ActiveTheme as _, Disableable as _, ElementExt as _, IconName,
    Sizable as _, VirtualListScrollHandle, WindowExt as _,
};
use rust_i18n::t;

#[path = "file_browser/context_menu.rs"]
mod context_menu;
#[path = "file_browser/actions.rs"]
mod actions;
#[path = "file_browser/render.rs"]
mod render;
#[path = "file_browser/render_views.rs"]
mod render_views;

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
    _subscription: Subscription,
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
    columns_horizontal_scroll_handle: ScrollHandle,
    columns_shell_bounds: Option<Bounds<Pixels>>,
    columns_horizontal_overflow: bool,
    columns_horizontal_column_count: usize,
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
    column_sweep_bounds: BTreeMap<usize, Bounds<Pixels>>,
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
            columns_horizontal_scroll_handle: ScrollHandle::default(),
            columns_shell_bounds: None,
            columns_horizontal_overflow: false,
            columns_horizontal_column_count: 0,
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
            column_sweep_bounds: BTreeMap::new(),
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

    fn operation_directory(&self) -> PathBuf {
        if self.view_mode == ViewMode::Columns
            && self.browse_location == BrowseLocation::Directory
        {
            if let Some((col_index, _)) = self.column_selected_path.as_ref() {
                if let Some(path) = self.column_trail.get(*col_index) {
                    return path.clone();
                }
            }
            if let Some(col_index) = self.active_column_index {
                if let Some(path) = self.column_trail.get(col_index) {
                    return path.clone();
                }
            }
            if let Some(parent) = self.selected_paths_common_parent() {
                return parent;
            }
        }

        self.current_dir.clone()
    }

    fn selected_paths_common_parent(&self) -> Option<PathBuf> {
        let mut paths = self.selected_paths.iter();
        let first_parent = paths.next()?.parent()?.to_path_buf();
        paths.all(|path| path.parent() == Some(first_parent.as_path()))
            .then_some(first_parent)
    }

    pub(crate) fn shows_directory(&self, path: &Path) -> bool {
        if self.current_dir == path {
            return true;
        }

        self.view_mode == ViewMode::Columns
            && self.browse_location == BrowseLocation::Directory
            && self.column_trail.iter().any(|entry| entry == path)
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
        let dest = self.operation_directory();
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
        let Some(bounds) = self.column_sweep_bounds.get(&col_index).copied() else {
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
        let bounds = self.column_sweep_bounds.get(&col_index).copied()?;
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

    fn update_columns_horizontal_scrollbar_state(
        &mut self,
        bounds: Bounds<Pixels>,
        _cx: &mut Context<Self>,
    ) -> bool {
        let overflow = COLUMN_WIDTH * self.column_trail.len().max(1) > bounds.size.width;
        let overflow_changed = self.columns_horizontal_overflow != overflow;
        let bounds_changed = self.columns_shell_bounds != Some(bounds);

        self.columns_shell_bounds = Some(bounds);
        self.columns_horizontal_overflow = overflow;
        self.columns_horizontal_column_count = self.column_trail.len();

        bounds_changed || overflow_changed
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
        if self.view_mode == ViewMode::Columns
            && self.browse_location == BrowseLocation::Directory
        {
            if let Some((selected_col, selected_path)) = self.column_selected_path.as_ref() {
                if let Some(items) = self.column_listings.get(*selected_col) {
                    if let Some(item) = items.iter().find(|item| item.path == *selected_path) {
                        return Some(item);
                    }
                }
            }
        }

        if self.selected_paths.len() == 1 {
            let path = self.selected_paths.iter().next()?;
            return self
                .display_items
                .iter()
                .find(|item| &item.path == path)
                .or_else(|| {
                    self.column_listings
                        .iter()
                        .flat_map(|list| list.iter())
                        .find(|item| &item.path == path)
                });
        }

        if self.view_mode == ViewMode::Columns
            && self.browse_location == BrowseLocation::Directory
            && self.selected_paths.is_empty()
        {
            return self
                .column_listings
                .iter()
                .enumerate()
                .rev()
                .find_map(|(col_index, items)| {
                    let selected_path = self.column_trail.get(col_index + 1)?;
                    items.iter().find(|item| item.path == *selected_path)
                });
        }

        None
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
        let browser = cx.entity().clone();
        let rename_path = path.clone();
        let subscription = cx.subscribe(&input, move |_, _, event: &InputEvent, cx| match event {
            InputEvent::Focus => {
                cx.defer(move |cx| {
                    let Some(window) = cx.active_window() else {
                        return;
                    };
                    let _ = window.update(cx, |_, window, cx| {
                        window.dispatch_action(Box::new(InputSelectAll), cx);
                    });
                });
            }
            InputEvent::PressEnter { .. } => {
                cx.stop_propagation();
                let browser = browser.clone();
                let rename_path = rename_path.clone();
                cx.defer(move |cx| {
                    let Some(window) = cx.active_window() else {
                        return;
                    };
                    let _ = window.update(cx, |_, window, cx| {
                        let _ = browser.update(cx, |this, cx| {
                            if this
                                .renaming
                                .as_ref()
                                .is_some_and(|renaming| renaming.path == rename_path)
                            {
                                this.commit_rename(window, cx);
                                cx.notify();
                            }
                        });
                    });
                });
            }
            InputEvent::Blur => {
                let browser = browser.clone();
                let rename_path = rename_path.clone();
                cx.defer(move |cx| {
                    let Some(window) = cx.active_window() else {
                        return;
                    };
                    let _ = window.update(cx, |_, _, cx| {
                        let _ = browser.update(cx, |this, cx| {
                            if this
                                .renaming
                                .as_ref()
                                .is_some_and(|renaming| renaming.path == rename_path)
                            {
                                this.cancel_rename();
                                cx.notify();
                            }
                        });
                    });
                });
            }
            _ => {}
        });
        input.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        self.renaming = Some(RenameState {
            path,
            input,
            _subscription: subscription,
        });
    }

    fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = self.renaming.take() else {
            return;
        };
        let new_name = renaming.input.read(cx).value();
        match rename_path(&renaming.path, &new_name) {
            Ok(target) => {
                self.error = None;
                let location_changed = self.rewrite_paths_after_rename(&renaming.path, &target);
                if self.selected_paths.remove(&renaming.path) {
                    self.selected_paths.insert(target);
                }
                self.refresh();
                if location_changed || self.watched_dir.as_ref() != Some(&self.current_dir) {
                    self.watched_dir = Some(self.current_dir.clone());
                    self.restart_directory_watcher(cx);
                }
                if location_changed {
                    Self::emit_location_changed(cx);
                }
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

    fn cancel_rename_if_active(&mut self, cx: &mut Context<Self>) {
        if self.renaming.is_some() {
            self.cancel_rename();
            cx.notify();
        }
    }

    fn renaming_input_for(&self, path: &Path) -> Option<Entity<InputState>> {
        self.renaming
            .as_ref()
            .filter(|renaming| renaming.path == path)
            .map(|renaming| renaming.input.clone())
    }

    fn inline_name_editor(
        input: Entity<InputState>,
        centered: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = if centered {
            Input::new(&input)
                .appearance(false)
                .small()
                .text_center()
                .into_any_element()
        } else {
            Input::new(&input)
                .appearance(false)
                .small()
                .w_full()
                .into_any_element()
        };

        div()
            .w_full()
            .min_w_0()
            .px_1()
            .py(px(1.))
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().primary)
            .bg(cx.theme().background)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(input)
            .into_any_element()
    }

    fn rewrite_paths_after_rename(&mut self, from: &Path, to: &Path) -> bool {
        let mut location_changed = false;

        if let Some(path) = renamed_path(&self.current_dir, from, to) {
            self.current_dir = path;
            location_changed = true;
        }

        rewrite_path_list(&mut self.back_stack, from, to);
        rewrite_path_list(&mut self.forward_stack, from, to);
        rewrite_path_list(&mut self.column_trail, from, to);

        if let Some(path) = self
            .watched_dir
            .as_ref()
            .and_then(|path| renamed_path(path, from, to))
        {
            self.watched_dir = Some(path);
        }

        self.selected_paths = self
            .selected_paths
            .iter()
            .map(|path| renamed_path(path, from, to).unwrap_or_else(|| path.clone()))
            .collect();

        if let Some((col_index, path)) = self.column_selected_path.as_ref() {
            if let Some(path) = renamed_path(path, from, to) {
                self.column_selected_path = Some((*col_index, path));
            }
        }

        location_changed
    }

    fn create_folder_from_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        let destination = self.operation_directory();
        let name = unique_new_folder_name(&destination);
        match create_directory(&destination, &name) {
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
        let destination = self.operation_directory();
        let name = unique_new_folder_name(&destination);
        match create_directory(&destination, &name) {
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
        let destination = self.operation_directory();
        let name = unique_new_file_name(&destination);
        match create_file(&destination, &name) {
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
        let destination = self.operation_directory();
        let zip_path = match unique_zip_output_path(&paths, &destination) {
            Ok(path) => path,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let partial_path = temp_zip_output_path(&zip_path);
        let partial_created = match create_compress_partial_file(&partial_path) {
            Ok(created) => created,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        self.refresh();
        spawn_compress(
            cx.entity(),
            window,
            cx,
            paths,
            destination,
            zip_path,
            partial_path,
            partial_created,
        );
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

        let destination = self.operation_directory();
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

}

impl Focusable for FileBrowser {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
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

fn create_compress_partial_file(path: &Path) -> anyhow::Result<bool> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn renamed_path(path: &Path, from: &Path, to: &Path) -> Option<PathBuf> {
    if path == from {
        return Some(to.to_path_buf());
    }

    let suffix = path.strip_prefix(from).ok()?;
    Some(to.join(suffix))
}

fn rewrite_path_list(paths: &mut Vec<PathBuf>, from: &Path, to: &Path) {
    for path in paths.iter_mut() {
        if let Some(updated) = renamed_path(path, from, to) {
            *path = updated;
        }
    }
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
