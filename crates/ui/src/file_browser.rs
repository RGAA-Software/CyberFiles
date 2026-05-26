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
#[path = "file_browser/rename.rs"]
mod rename;
#[path = "file_browser/selection.rs"]
mod selection;
#[path = "file_browser/ops.rs"]
mod ops;
#[path = "file_browser/navigation.rs"]
mod navigation;
#[path = "file_browser/sweep.rs"]
mod sweep;
#[path = "file_browser/context_menu_state.rs"]
mod context_menu_state;

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

    pub fn read_options(&self) -> &DirectoryReadOptions {
        &self.read_options
    }

    pub fn current_directory(&self) -> &PathBuf {
        &self.current_dir
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
