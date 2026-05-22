use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use chrono::{DateTime, Local};
use cyberfiles_commands::{
    CopyItems, CopyPath, CutItems, DeleteItems, DeleteItemsPermanent, FocusSearch, NavigateNext,
    NavigatePrevious, NewFile, NewFolder, OpenItem,
    PasteItems, RefreshDirectory, RenameItem, SelectAll, ShellProperties, ViewColumns, ViewDetails,
    ViewGrid, FILE_BROWSER,
};
use cyberfiles_core::{
    file_sort_prefs_from_config, file_view_mode_from_config, load_config, save_file_browser_prefs,
    VIEW_COLUMNS, VIEW_DETAILS, VIEW_GRID,
};
use cyberfiles_fs::{
    column_trail_for, copy_items, create_directory, create_file, delete_paths,
    file_items_for_tag_paths, filter_items_by_query, home_navigation_path, move_items,
    read_directory, read_recycle_bin,
    recycle_paths, rename_path, unique_new_file_name, unique_new_folder_name, ClipboardOperation,
    DirectoryReadOptions, DirectoryWatcher, FileClipboard, FileItem, FileItemKind, SortDirection,
    SortOption, SortPreferences,
};
use crate::app_state::AppNavigation;
use crate::icons::{compact_icon, toolbar_icon};
use crate::toolbar_button::{toolbar_dropdown_button, toolbar_icon_button, toolbar_labeled_button};
use cyberfiles_platform_windows::{self as platform, ShellContextMenuEntry, ShellIconHint};
use crate::app_state::AppFileClipboard;
use gpui::{
    actions, prelude::*, ClipboardItem, ClickEvent, Entity, FocusHandle,
    Focusable, ParentElement, ScrollStrategy, Subscription, Window, *,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    dialog::DialogButtonProps,
    h_flex,
    input::{Input, InputState},
    menu::ContextMenuExt,
    notification::Notification,
    scroll::{ScrollableElement as _, ScrollbarAxis},
    v_flex, v_virtual_list, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    VirtualListScrollHandle, WindowExt as _,
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
        OpenInNewPane,
        OpenInNewWindow,
        OpenInTerminal,
        OpenWithDialog,
        CreateFolderFromSelection,
        CreateShortcut,
    ]
);

const FILE_ROW_SIZE: Size<Pixels> = size(px(1.), px(36.));
const GRID_CELL_SIZE: Size<Pixels> = size(px(112.), px(96.));
const COLUMN_ROW_SIZE: Size<Pixels> = size(px(1.), px(32.));
const COLUMN_WIDTH: Pixels = px(200.);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Details,
    Grid,
    Columns,
}

impl ViewMode {
    fn from_config(value: &str) -> Self {
        match value {
            VIEW_GRID => Self::Grid,
            VIEW_COLUMNS => Self::Columns,
            _ => Self::Details,
        }
    }

    fn config_value(self) -> &'static str {
        match self {
            Self::Details => VIEW_DETAILS,
            Self::Grid => VIEW_GRID,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrowseLocation {
    Directory,
    RecycleBin,
    FileTag { tag_name: String },
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
    error: Option<String>,
    selected_paths: BTreeSet<PathBuf>,
    anchor_index: Option<usize>,
    focused_index: Option<usize>,
    renaming: Option<RenameState>,
    show_toolbar: bool,
    /// View/sort/actions row (Files `InnerNavigationToolbar`), below window nav + omnibar.
    show_content_toolbar: bool,
    view_mode: ViewMode,
    search_query: String,
    display_items: Vec<FileItem>,
    column_trail: Vec<PathBuf>,
    column_listings: Vec<Vec<FileItem>>,
    _directory_watcher: Option<DirectoryWatcher>,
    _watcher_task: Option<Task<()>>,
    watched_dir: Option<PathBuf>,
    shell_menu_cache: Option<ShellMenuCache>,
    _shell_menu_task: Option<Task<()>>,
    context_menu_extended_verbs: bool,
    _subscriptions: Vec<Subscription>,
}

impl FileBrowser {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::with_options(cx, home_navigation_path(), true, false)
    }

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
        let (sort_option, sort_direction, show_hidden) = file_sort_prefs_from_config();
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
        }

        let view_mode = ViewMode::from_config(&file_view_mode_from_config());
        let (items, error) = load_files_dir(&current_dir, read_options, sort_preferences);
        let display_items = filter_items_by_query(&items, "");
        let column_trail = column_trail_for(&current_dir);
        let column_listings =
            column_listings_for(&column_trail, &read_options, sort_preferences, "");

        Self {
            focus_handle: cx.focus_handle(),
            browse_location: BrowseLocation::Directory,
            current_dir,
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            item_sizes: item_sizes_for(display_items.len(), ViewMode::Details),
            scroll_handle: VirtualListScrollHandle::new(),
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
            view_mode,
            search_query: String::new(),
            display_items,
            column_trail,
            column_listings,
            _directory_watcher: None,
            _watcher_task: None,
            watched_dir: None,
            shell_menu_cache: None,
            _shell_menu_task: None,
            context_menu_extended_verbs: false,
            _subscriptions: Vec::new(),
        }
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
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
        self.item_sizes = item_sizes_for(self.display_items.len(), self.view_mode);
        if self.view_mode == ViewMode::Columns {
            self.refresh_column_listings();
        }
        self.clamp_focused_index();
    }

    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        if self.view_mode != mode {
            self.view_mode = mode;
            self.item_sizes = item_sizes_for(self.display_items.len(), self.view_mode);
            if mode == ViewMode::Columns {
                self.refresh_column_listings();
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
        );
    }

    fn refresh_column_listings(&mut self) {
        self.column_trail = column_trail_for(&self.current_dir);
        self.column_listings = column_listings_for(
            &self.column_trail,
            &self.read_options,
            self.sort_preferences,
            &self.search_query,
        );
    }

    /// Prefetch Shell context menu entries off the UI thread (Files-style flyout merge).
    fn request_shell_menu_fetch(&mut self, cx: &mut Context<Self>) {
        if self.browse_location != BrowseLocation::Directory {
            return;
        }

        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }

        let extended = self.context_menu_extended_verbs;
        if self.shell_menu_cache.as_ref().is_some_and(|cache| {
            cache.paths == paths && cache.extended_verbs == extended
        }) {
            return;
        }

        self._shell_menu_task.take();
        self._shell_menu_task = Some(cx.spawn(async move |browser, cx| {
            let paths_for_query = paths.clone();
            let entries = cx.background_spawn(async move {
                platform::query_shell_context_menu_items(&paths_for_query, extended)
                    .unwrap_or_default()
            }).await;

            let _ = browser.update(cx, |browser, cx| {
                browser.shell_menu_cache = Some(ShellMenuCache {
                    paths,
                    extended_verbs: extended,
                    entries,
                });
                cx.notify();
            });
        }));
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

    pub fn open_directory(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.navigate_to(path, cx);
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
            BrowseLocation::RecycleBin => match read_recycle_bin(
                self.read_options,
                self.sort_preferences,
            ) {
                Ok(items) => (items, None),
                Err(error) => (Vec::new(), Some(error.to_string())),
            },
            BrowseLocation::FileTag { tag_name } => {
                let paths = paths_for_file_tag(tag_name);
                if paths.is_empty() {
                    (
                        Vec::new(),
                        Some(t!("file_tag.empty").to_string()),
                    )
                } else {
                    (
                        file_items_for_tag_paths(
                            &paths,
                            self.read_options,
                            self.sort_preferences,
                        ),
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
        let result = if copy {
            copy_items(&paths, &dest)
        } else {
            move_items(&paths, &dest)
        };
        match result {
            Ok(()) => {
                self.refresh();
                cx.notify();
            }
            Err(error) => {
                window.push_notification(
                    Notification::error(format!("{}: {error}", t!("files.drop.error"))),
                    cx,
                );
            }
        }
    }

    fn drag_paths_for_item(&self, _index: usize, path: &Path) -> Vec<PathBuf> {
        if self.selected_paths.contains(path) && !self.selected_paths.is_empty() {
            return self.selected_paths_vec();
        }
        vec![path.to_path_buf()]
    }

    fn select_column_item(&mut self, col_index: usize, item: &FileItem, cx: &mut Context<Self>) {
        match item.kind {
            FileItemKind::Folder => {
                if self.current_dir != item.path {
                    self.back_stack.push(self.current_dir.clone());
                    self.forward_stack.clear();
                }
                self.current_dir = item.path.clone();
                self.column_trail.truncate(col_index + 1);
                self.column_trail.push(item.path.clone());
                self.clear_selection();
                self.refresh();
                Self::emit_location_changed(cx);
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                self.open_item(item.path.clone(), item.kind, cx);
            }
        }
    }

    fn column_selection_name(&self, col_index: usize) -> Option<String> {
        let next = self.column_trail.get(col_index + 1)?;
        next.file_name().map(|n| n.to_string_lossy().to_string())
    }

    fn clear_shell_menu_cache(&mut self) {
        self.shell_menu_cache = None;
        self._shell_menu_task.take();
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
    }

    fn handle_row_click(&mut self, index: usize, event: &ClickEvent, cx: &mut Context<Self>) {
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
        self.request_shell_menu_fetch(cx);
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
        let next = (index as isize + delta)
            .clamp(0, self.display_items.len() as isize - 1) as usize;
        self.focused_index = Some(next);
        self.scroll_handle
            .scroll_to_item(next, ScrollStrategy::Center);
    }

    fn select_all(&mut self) {
        self.selected_paths = self
            .display_items
            .iter()
            .map(|item| item.path.clone())
            .collect();
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
        self.display_items.iter().find(|item| &item.path == path)
    }

    fn primary_path(&self) -> Option<PathBuf> {
        self.primary_selected_item()
            .map(|item| item.path.clone())
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
        let input = cx.new(|cx| {
            InputState::new(window, cx).default_value(default_name)
        });
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
                    window.push_notification(Notification::success(t!("files.new_folder.success")), cx);
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
                    window.push_notification(Notification::success(t!("files.new_file.success")), cx);
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

    fn copy_items(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        AppFileClipboard::store(ClipboardOperation::Copy, paths, cx);
    }

    fn cut_items(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        AppFileClipboard::store(ClipboardOperation::Cut, paths, cx);
    }

    fn paste_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

        let destination = self.current_dir.clone();
        let result = match clipboard.operation {
            ClipboardOperation::Copy => copy_items(&clipboard.paths, &destination),
            ClipboardOperation::Cut => move_items(&clipboard.paths, &destination),
        };

        match result {
            Ok(()) => {
                if clipboard.operation == ClipboardOperation::Copy {
                    AppFileClipboard::store(clipboard.operation, clipboard.paths, cx);
                }
                self.refresh();
                window.push_notification(Notification::success(t!("files.paste.success")), cx);
            }
            Err(error) => {
                AppFileClipboard::set(clipboard, cx);
                window.push_notification(
                    Notification::error(format!("{}: {error}", t!("files.paste.error"))),
                    cx,
                );
            }
        }
        cx.notify();
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
        let paths = std::rc::Rc::new(paths);
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
                .on_ok(move |_dialog, window, cx| {
                    let success = success.clone();
                    let delete_result = if permanent {
                        delete_paths(paths.as_ref())
                    } else {
                        recycle_paths(paths.as_ref())
                    };
                    match delete_result {
                        Ok(()) => {
                            browser.update(cx, |browser, cx| {
                                browser.clear_selection();
                                browser.refresh();
                                cx.notify();
                            });
                            window.push_notification(Notification::success(success), cx);
                            true
                        }
                        Err(error) => {
                            window.push_notification(
                                Notification::error(format!(
                                    "{}: {error}",
                                    t!("files.delete.error")
                                )),
                                cx,
                            );
                            false
                        }
                    }
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

    fn file_list(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.view_mode {
            ViewMode::Details => self.details_table(window, cx).into_any_element(),
            ViewMode::Grid => self.grid_view(window, cx).into_any_element(),
            ViewMode::Columns => self.columns_view(window, cx).into_any_element(),
        }
    }

    fn columns_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let columns = self
            .column_trail
            .iter()
            .enumerate()
            .zip(self.column_listings.iter())
            .map(|((col_index, col_path), items)| {
                let title = col_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| col_path.to_string_lossy().to_string());
                let selected_name = self.column_selection_name(col_index);
                let cells = items
                    .iter()
                    .map(|item| {
                        let item = item.clone();
                        let is_selected =
                            selected_name.as_deref() == Some(item.display_name.as_str());
                        let drag_paths = vec![item.path.clone()];
                        Self::column_cell(window, col_index, item, is_selected, drag_paths, cx)
                    })
                    .collect::<Vec<_>>();

                v_flex()
                    .id(("files-column", col_index))
                    .w(COLUMN_WIDTH)
                    .flex_none()
                    .flex_1()
                    .min_h_0()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .h_8()
                            .px_2()
                            .flex_none()
                            .items_center()
                            .bg(cx.theme().muted)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(title),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .children(cells),
                    )
            })
            .collect::<Vec<_>>();

        h_flex()
            .id("files-columns-wrap")
            .size_full()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_x_scroll()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.clear_selection();
                cx.notify();
            }))
            .children(columns)
    }

    fn column_cell(
        window: &mut Window,
        col_index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let kind = item.kind;
        let name = item.display_name.clone();
        let item_click = item.clone();
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
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if kind == FileItemKind::Folder {
                    this.select_column_item(col_index, &item_click, cx);
                } else if event.click_count() == 2 {
                    this.open_item(item_click.path.clone(), kind, cx);
                } else {
                    cx.notify();
                }
            }))
            .on_drag(DraggedFilePaths(drag_paths), |paths, _offset, _window, cx| {
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
            })
            .child(
                div()
                    .w(px(20.))
                    .flex_none()
                    .child(crate::shell_icon::shell_icon_for_path(
                        &item.path,
                        px(16.),
                        window,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
            .into_any_element()
    }

    fn details_table(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                                        let selected =
                                            this.selected_paths.contains(&item.path);
                                        let drag_paths =
                                            this.drag_paths_for_item(index, &item.path);
                                        Some(Self::row(
                                            window,
                                            index,
                                            item,
                                            selected,
                                            drag_paths,
                                            cx,
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

    fn grid_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cells = self
            .display_items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let item = item.clone();
                let selected = self.selected_paths.contains(&item.path);
                let drag_paths = self.drag_paths_for_item(index, &item.path);
                Self::grid_cell(window, index, item, selected, drag_paths, cx)
            })
            .collect::<Vec<_>>();

        v_flex()
            .id("files-grid-view")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .child(
                div()
                    .id("files-grid-wrap")
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .overflow_y_scroll()
                    .p_2()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(cells),
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
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.request_shell_menu_fetch(cx);
                    cx.notify();
                }),
            )
            .on_drag(DraggedFilePaths(drag_paths), move |paths, _offset, _window, cx| {
                cx.new(|_| DragPathPreview {
                    label: drag_preview_label(&paths.0).into(),
                })
            })
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::shell_icon::shell_icon_for_path(
                        &item.path,
                        px(16.),
                        window,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open_path = item.path.clone();
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let name = item.display_name.clone();
        v_flex()
            .id(("file-grid-cell", index))
            .w(px(112.))
            .h(px(96.))
            .flex_none()
            .p_2()
            .gap_1()
            .items_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.request_shell_menu_fetch(cx);
                    cx.notify();
                }),
            )
            .on_drag(DraggedFilePaths(drag_paths), move |paths, _offset, _window, cx| {
                cx.new(|_| DragPathPreview {
                    label: drag_preview_label(&paths.0).into(),
                })
            })
            .child(crate::shell_icon::shell_icon_for_path(
                &item.path,
                px(16.),
                window,
            ))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_xs()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
            .child(
                toolbar_icon_button(format!("grid-open-{index}"))
                    .icon(toolbar_icon(IconName::ExternalLink))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_item(open_path.clone(), kind, cx);
                    })),
            )
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
        self.request_shell_menu_fetch(cx);
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

    fn on_open_in_new_pane(&mut self, _: &OpenInNewPane, _: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.primary_path() else {
            return;
        };
        AppNavigation::open_path_in_secondary_pane(path, cx);
    }

    fn on_open_in_terminal(&mut self, _: &OpenInTerminal, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.primary_path() else {
            return;
        };
        if let Err(error) = open_path_in_terminal(&path) {
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
        let Some(path) = self.primary_path() else {
            return;
        };
        if let Err(error) = create_shortcut_for_path(&path) {
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

    fn on_view_columns(&mut self, _: &ViewColumns, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Columns, cx);
    }

    fn on_focus_search_action(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_search(window, cx);
    }

    fn on_shell_properties(&mut self, _: &ShellProperties, _window: &mut Window, cx: &mut Context<Self>) {
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
        let sort_label = self.sort_label();

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
            .child(
                toolbar_icon_button("content-new-folder")
                    .icon(toolbar_icon(IconName::Folder))
                    .tooltip(t!("files.new_folder"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.create_new_folder(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-new-file")
                    .icon(toolbar_icon(IconName::File))
                    .tooltip(t!("files.new_file"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.create_new_file(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-view-details")
                    .icon(toolbar_icon(IconName::GalleryVerticalEnd))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Details, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-grid")
                    .icon(toolbar_icon(IconName::LayoutDashboard))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Grid, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-columns")
                    .icon(toolbar_icon(IconName::PanelLeft))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Columns, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-delete")
                    .icon(toolbar_icon(IconName::Delete))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.perform_delete(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_dropdown_button("content-sort")
                    .button(toolbar_labeled_button("content-sort-btn").label(sort_label))
                    .dropdown_menu(move |menu, _, _| {
                        let hidden_label = if show_hidden {
                            t!("files.show_hidden.off")
                        } else {
                            t!("files.show_hidden.on")
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

        let current_dir = self.current_dir.to_string_lossy().to_string();
        let can_go_back = !self.back_stack.is_empty();
        let can_go_forward = !self.forward_stack.is_empty();
        let can_go_up = self.current_dir.parent().is_some();
        let selected_count = self.selected_paths.len();
        let show_hidden = self.read_options.show_hidden_items;
        let sort_label = self.sort_label();
        let in_recycle_bin = self.browse_location == BrowseLocation::RecycleBin;
        let browser = cx.entity();

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
            .on_action(cx.listener(Self::on_view_grid))
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
            .on_action(cx.listener(Self::on_navigate_previous))
            .on_action(cx.listener(Self::on_navigate_next))
            .on_action(cx.listener(Self::on_sort_name))
            .on_action(cx.listener(Self::on_sort_created))
            .on_action(cx.listener(Self::on_sort_modified))
            .on_action(cx.listener(Self::on_sort_size))
            .on_action(cx.listener(Self::on_sort_type))
            .on_action(cx.listener(Self::on_toggle_sort_direction))
            .on_action(cx.listener(Self::on_toggle_show_hidden))
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
                            .disabled(!can_go_back)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_back(cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("files-forward")
                            .icon(toolbar_icon(IconName::ArrowRight))
                            .disabled(!can_go_forward)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_forward(cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("files-up")
                            .icon(toolbar_icon(IconName::ArrowUp))
                            .disabled(!can_go_up)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_parent(cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("files-refresh")
                            .icon(toolbar_icon(IconName::Redo2))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh();
                                cx.notify();
                            })),
                    )
                    .child(
                        toolbar_icon_button("files-new-folder-btn")
                            .icon(toolbar_icon(IconName::Folder))
                            .tooltip(t!("files.new_folder"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_new_folder(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        toolbar_icon_button("files-new-file-btn")
                            .icon(toolbar_icon(IconName::File))
                            .tooltip(t!("files.new_file"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_new_file(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        toolbar_icon_button("files-view-details")
                            .icon(toolbar_icon(IconName::GalleryVerticalEnd))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_view_mode(ViewMode::Details, cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("files-view-grid")
                            .icon(toolbar_icon(IconName::LayoutDashboard))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_view_mode(ViewMode::Grid, cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("files-view-columns")
                            .icon(toolbar_icon(IconName::PanelLeft))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_view_mode(ViewMode::Columns, cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("files-delete-btn")
                            .icon(toolbar_icon(IconName::Delete))
                            .disabled(selected_count == 0)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.perform_delete(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        toolbar_dropdown_button("files-sort")
                            .button(toolbar_labeled_button("files-sort-btn").label(sort_label))
                            .dropdown_menu(move |menu, _, _| {
                                let hidden_label = if show_hidden {
                                    t!("files.show_hidden.off")
                                } else {
                                    t!("files.show_hidden.on")
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
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.clear_selection();
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.set_context_menu_extended_verbs(event.modifiers.shift);
                            cx.notify();
                        }),
                    )
                    .child(self.file_list(window, cx)),
            )
            .context_menu(move |menu, window, cx| {
                browser.update(cx, |browser, cx| {
                    browser.request_shell_menu_fetch(cx);
                });
                context_menu::build_context_menu(menu, browser.clone(), window, cx)
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

fn item_sizes_for(count: usize, mode: ViewMode) -> Rc<Vec<Size<Pixels>>> {
    let size = match mode {
        ViewMode::Details => FILE_ROW_SIZE,
        ViewMode::Grid => GRID_CELL_SIZE,
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
fn open_path_in_terminal(path: &Path) -> anyhow::Result<()> {
    use std::path::Path;
    use std::process::Command;

    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow::anyhow!("no parent directory"))?
    };
    let dir = dir.to_string_lossy();
    let wt = Command::new("wt.exe").args(["-d", &dir]).spawn();
    if wt.is_ok() {
        return Ok(());
    }
    Command::new("cmd")
        .args(["/C", "start", "", "wt.exe", "-d", &dir])
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn open_path_in_terminal(_path: &Path) -> anyhow::Result<()> {
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

fn sort_direction_config_value(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "asc",
        SortDirection::Descending => "desc",
    }
}

fn icon_for_item(item: &FileItem) -> IconName {
    match platform::icon_hint_for_path(&item.path) {
        ShellIconHint::Folder => IconName::Folder,
        ShellIconHint::Symlink => IconName::ExternalLink,
        ShellIconHint::Executable => IconName::Settings2,
        ShellIconHint::Image => IconName::File,
        ShellIconHint::Archive => IconName::Folder,
        ShellIconHint::File => IconName::File,
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
