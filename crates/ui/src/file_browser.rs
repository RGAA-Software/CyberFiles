use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use chrono::{DateTime, Local};
use cyberfiles_commands::{
    CopyItems, CopyPath, CutItems, DeleteItems, DeleteItemsPermanent, FocusSearch, NavigateBack,
    NavigateForward, NavigateNext, NavigatePrevious, NavigateUp, NewFile, NewFolder, OpenItem,
    PasteItems, RefreshDirectory, RenameItem, SelectAll, ShellContextMenu, ShellProperties,
    ViewDetails, ViewGrid, FILE_BROWSER,
};
use cyberfiles_fs::{
    copy_items, create_directory, create_file, delete_paths, filter_items_by_query,
    home_navigation_path, move_items, read_directory, recycle_paths, rename_path,
    unique_new_file_name, unique_new_folder_name, ClipboardOperation, DirectoryReadOptions,
    DirectoryWatcher, FileClipboard, FileItem, FileItemKind, SortDirection, SortOption,
    SortPreferences,
};
use cyberfiles_platform_windows::{self as platform, ShellIconHint};
use crate::app_state::AppFileClipboard;
use gpui::{
    actions, prelude::*, ClipboardItem, ClickEvent, Entity, FocusHandle,
    Focusable, ParentElement, ScrollStrategy, Subscription, Window, *,
};
use gpui_component::{
    button::{Button, ButtonVariants as _, DropdownButton},
    dialog::DialogButtonProps,
    h_flex,
    input::{Input, InputState},
    scroll::{ScrollableElement as _, ScrollbarAxis},
    v_flex, v_virtual_list, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    VirtualListScrollHandle, WindowExt as _,
};
use rust_i18n::t;

actions!(
    file_browser_prefs,
    [
        SortByName,
        SortByModified,
        SortBySize,
        SortByType,
        ToggleSortDirection,
        ToggleShowHidden,
    ]
);

const FILE_ROW_SIZE: Size<Pixels> = size(px(1.), px(36.));
const GRID_CELL_SIZE: Size<Pixels> = size(px(112.), px(96.));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Details,
    Grid,
}

struct RenameState {
    path: PathBuf,
    input: Entity<InputState>,
}

pub struct FileBrowser {
    focus_handle: FocusHandle,
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
    view_mode: ViewMode,
    search_query: String,
    display_items: Vec<FileItem>,
    _directory_watcher: Option<DirectoryWatcher>,
    _watcher_task: Option<Task<()>>,
    watched_dir: Option<PathBuf>,
    _subscriptions: Vec<Subscription>,
}

impl FileBrowser {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::with_options(cx, home_navigation_path(), true)
    }

    /// File list for embedding in MainPage (navigation chrome lives on the shell).
    pub fn for_shell(cx: &mut Context<Self>, initial_dir: PathBuf) -> Self {
        Self::with_options(cx, initial_dir, false)
    }

    fn with_options(cx: &mut Context<Self>, current_dir: PathBuf, show_toolbar: bool) -> Self {
        let (items, error) = load_files_dir(
            &current_dir,
            DirectoryReadOptions::default(),
            SortPreferences::default(),
        );
        let focused_index = items.first().map(|_| 0);
        let display_items = filter_items_by_query(&items, "");

        Self {
            focus_handle: cx.focus_handle(),
            current_dir,
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            item_sizes: item_sizes_for(display_items.len(), ViewMode::Details),
            scroll_handle: VirtualListScrollHandle::new(),
            items,
            read_options: DirectoryReadOptions::default(),
            sort_preferences: SortPreferences::default(),
            error,
            selected_paths: BTreeSet::new(),
            anchor_index: focused_index,
            focused_index,
            renaming: None,
            show_toolbar,
            view_mode: ViewMode::Details,
            search_query: String::new(),
            display_items,
            _directory_watcher: None,
            _watcher_task: None,
            watched_dir: None,
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
        // Search input lives on MainPage; FileBrowser only holds the query string.
        let _ = (window, cx);
    }

    fn apply_filter(&mut self) {
        self.display_items = filter_items_by_query(&self.items, &self.search_query);
        self.item_sizes = item_sizes_for(self.display_items.len(), self.view_mode);
        self.clamp_focused_index();
    }

    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        if self.view_mode != mode {
            self.view_mode = mode;
            self.item_sizes = item_sizes_for(self.display_items.len(), self.view_mode);
            cx.notify();
        }
    }

    fn restart_directory_watcher(&mut self, cx: &mut Context<Self>) {
        self._watcher_task.take();
        self._directory_watcher.take();

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

    pub fn current_directory(&self) -> &PathBuf {
        &self.current_dir
    }

    pub fn item_count(&self) -> usize {
        self.display_items.len()
    }

    pub fn selected_count(&self) -> usize {
        self.selected_paths.len().max(usize::from(
            self.selected_paths.is_empty() && self.focused_index.is_some(),
        ))
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

    pub fn go_back(&mut self) {
        self.navigate_back();
    }

    pub fn go_forward(&mut self) {
        self.navigate_forward();
    }

    pub fn go_up(&mut self) {
        self.navigate_parent();
    }

    pub fn reload(&mut self) {
        self.refresh();
    }

    pub fn open_directory(&mut self, path: PathBuf) {
        self.navigate_to(path);
    }

    pub fn open_directory_reset_history(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.back_stack.clear();
        self.forward_stack.clear();
        self.current_dir = path;
        self.clear_selection();
        self.refresh();
        self.restart_directory_watcher(cx);
    }

    fn refresh(&mut self) {
        let (items, error) =
            load_files_dir(&self.current_dir, self.read_options, self.sort_preferences);
        self.items = items;
        self.error = error;
        self.apply_filter();
        self.reconcile_selection();
        self.clamp_focused_index();
    }

    fn navigate_to(&mut self, path: PathBuf) {
        if path == self.current_dir {
            return;
        }

        self.back_stack.push(self.current_dir.clone());
        self.forward_stack.clear();
        self.current_dir = path;
        self.clear_selection();
        self.refresh();
    }

    fn navigate_back(&mut self) {
        let Some(path) = self.back_stack.pop() else {
            return;
        };

        self.forward_stack.push(self.current_dir.clone());
        self.current_dir = path;
        self.clear_selection();
        self.refresh();
    }

    fn navigate_forward(&mut self) {
        let Some(path) = self.forward_stack.pop() else {
            return;
        };

        self.back_stack.push(self.current_dir.clone());
        self.current_dir = path;
        self.clear_selection();
        self.refresh();
    }

    fn navigate_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.navigate_to(parent.to_path_buf());
        }
    }

    fn clear_selection(&mut self) {
        self.selected_paths.clear();
        self.anchor_index = None;
        self.focused_index = self.display_items.first().map(|_| 0);
    }

    fn handle_row_click(&mut self, index: usize, event: &ClickEvent) {
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

    fn open_item(&mut self, path: PathBuf, kind: FileItemKind) {
        match kind {
            FileItemKind::Folder => self.navigate_to(path),
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                if let Err(error) = open_with_system(&path) {
                    self.error = Some(error.to_string());
                }
            }
        }
    }

    fn open_focused(&mut self) {
        let Some(index) = self.focused_index else {
            return;
        };
        let Some(item) = self.display_items.get(index) else {
            return;
        };
        self.open_item(item.path.clone(), item.kind);
    }

    fn reconcile_selection(&mut self) {
        self.selected_paths
            .retain(|path| self.display_items.iter().any(|item| &item.path == path));
        if self.selected_paths.is_empty() {
            if let Some(index) = self.focused_index {
                if index >= self.display_items.len() {
                    self.focused_index = self.display_items.first().map(|_| 0);
                }
            }
        }
    }

    fn clamp_focused_index(&mut self) {
        if self.display_items.is_empty() {
            self.focused_index = None;
            return;
        }
        if self.focused_index.is_none() {
            self.focused_index = Some(0);
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
        if self.selected_paths.len() == 1 {
            let path = self.selected_paths.iter().next()?;
            return self.display_items.iter().find(|item| &item.path == path);
        }
        if !self.selected_paths.is_empty() {
            return None;
        }
        let index = self.focused_index?;
        self.display_items.get(index)
    }

    fn primary_path(&self) -> Option<PathBuf> {
        self.primary_selected_item()
            .map(|item| item.path.clone())
            .or_else(|| {
                if let Some(index) = self.focused_index {
                    return self
                        .display_items
                        .get(index)
                        .map(|item| item.path.clone());
                }
                self.selected_paths.iter().next().cloned()
            })
    }

    fn selected_paths_vec(&self) -> Vec<PathBuf> {
        if !self.selected_paths.is_empty() {
            return self.selected_paths.iter().cloned().collect();
        }
        self.primary_path().into_iter().collect()
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
                window.push_notification(SharedString::from(t!("files.rename.success")), cx);
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
                        SharedString::from(t!("files.new_folder.success")),
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
                    window.push_notification(SharedString::from(t!("files.new_file.success")), cx);
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
                window.push_notification(SharedString::from(t!("files.paste.success")), cx);
            }
            Err(error) => {
                AppFileClipboard::set(clipboard, cx);
                window.push_notification(
                    SharedString::from(format!("{}: {error}", t!("files.paste.error"))),
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
                            window.push_notification(success, cx);
                            true
                        }
                        Err(error) => {
                            window.push_notification(
                                SharedString::from(format!(
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

    fn file_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        match self.view_mode {
            ViewMode::Details => self.details_table(cx).into_any_element(),
            ViewMode::Grid => self.grid_view(cx).into_any_element(),
        }
    }

    fn details_table(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
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
                            |this, visible_range, _, cx| {
                                let focused_index = this.focused_index;
                                visible_range
                                    .filter_map(|index| {
                                        let item = this.display_items.get(index)?.clone();
                                        let selected =
                                            this.selected_paths.contains(&item.path);
                                        let focused = focused_index == Some(index);
                                        Some(Self::row(index, item, selected, focused, cx))
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    )
                    .scrollbar(&self.scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn grid_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let cells = self
            .display_items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let item = item.clone();
                let selected = self.selected_paths.contains(&item.path);
                let focused = self.focused_index == Some(index);
                Self::grid_cell(index, item, selected, focused, cx)
            })
            .collect::<Vec<_>>();

        v_flex()
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
                    .overflow_y_scroll()
                    .p_2()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(cells),
            )
    }

    fn row(
        index: usize,
        item: FileItem,
        selected: bool,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open_path = item.path.clone();
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let icon = icon_for_item(&item);
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
            .when(selected || focused, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind);
                } else {
                    this.handle_row_click(index, event);
                }
                cx.notify();
            }))
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(icon).small()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
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
                    Button::new(format!("open-item-{index}"))
                        .xsmall()
                        .ghost()
                        .icon(match kind {
                            FileItemKind::Folder => IconName::ChevronRight,
                            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                                IconName::ExternalLink
                            }
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_item(open_path.clone(), kind);
                            cx.notify();
                        })),
                ),
            )
            .into_any_element()
    }

    fn grid_cell(
        index: usize,
        item: FileItem,
        selected: bool,
        focused: bool,
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
            .when(selected || focused, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind);
                } else {
                    this.handle_row_click(index, event);
                }
                cx.notify();
            }))
            .child(Icon::new(icon_for_item(&item)).small())
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
                Button::new(format!("grid-open-{index}"))
                    .xsmall()
                    .ghost()
                    .icon(IconName::ExternalLink)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_item(open_path.clone(), kind);
                        cx.notify();
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
    fn on_navigate_back(&mut self, _: &NavigateBack, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate_back();
        cx.notify();
    }

    fn on_navigate_forward(&mut self, _: &NavigateForward, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate_forward();
        cx.notify();
    }

    fn on_navigate_up(&mut self, _: &NavigateUp, _: &mut Window, cx: &mut Context<Self>) {
        self.navigate_parent();
        cx.notify();
    }

    fn on_refresh(&mut self, _: &RefreshDirectory, _: &mut Window, cx: &mut Context<Self>) {
        self.refresh();
        cx.notify();
    }

    fn on_open_item(&mut self, _: &OpenItem, _: &mut Window, cx: &mut Context<Self>) {
        self.open_focused();
        cx.notify();
    }

    fn on_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all();
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
        cx.notify();
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

    fn on_shell_properties(&mut self, _: &ShellProperties, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.primary_path() else {
            return;
        };
        if let Err(error) = platform::open_item_properties(&path) {
            window.push_notification(
                SharedString::from(format!("{}: {error}", t!("files.properties.error"))),
                cx,
            );
        }
    }

    fn on_shell_context_menu(
        &mut self,
        _: &ShellContextMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        if let Err(error) = platform::show_shell_context_menu(&paths) {
            window.push_notification(
                SharedString::from(format!("{}: {error}", t!("files.context_menu.error"))),
                cx,
            );
        }
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
        let selected_count = self.selected_paths.len().max(usize::from(
            self.selected_paths.is_empty() && self.focused_index.is_some(),
        ));
        let show_hidden = self.read_options.show_hidden_items;
        let sort_label = self.sort_label();

        v_flex()
            .id("files-page")
            .size_full()
            .min_h_0()
            .gap_3()
            .track_focus(&self.focus_handle)
            .key_context(FILE_BROWSER)
            .on_action(cx.listener(Self::on_navigate_back))
            .on_action(cx.listener(Self::on_navigate_forward))
            .on_action(cx.listener(Self::on_navigate_up))
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
            .on_action(cx.listener(Self::on_shell_properties))
            .on_action(cx.listener(Self::on_shell_context_menu))
            .on_action(cx.listener(Self::on_copy_path))
            .on_action(cx.listener(Self::on_copy_items))
            .on_action(cx.listener(Self::on_cut_items))
            .on_action(cx.listener(Self::on_paste_items))
            .on_action(cx.listener(Self::on_navigate_previous))
            .on_action(cx.listener(Self::on_navigate_next))
            .on_action(cx.listener(Self::on_sort_name))
            .on_action(cx.listener(Self::on_sort_modified))
            .on_action(cx.listener(Self::on_sort_size))
            .on_action(cx.listener(Self::on_sort_type))
            .on_action(cx.listener(Self::on_toggle_sort_direction))
            .on_action(cx.listener(Self::on_toggle_show_hidden))
            .when(self.show_toolbar, |this| {
                this.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .flex_wrap()
                    .child(
                        Button::new("files-back")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowLeft)
                            .disabled(!can_go_back)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_back();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("files-forward")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowRight)
                            .disabled(!can_go_forward)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_forward();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("files-up")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowUp)
                            .disabled(!can_go_up)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_parent();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("files-refresh")
                            .small()
                            .ghost()
                            .icon(IconName::Redo2)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("files-new-folder-btn")
                            .small()
                            .outline()
                            .icon(IconName::Folder)
                            .label(t!("files.new_folder"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_new_folder(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("files-new-file-btn")
                            .small()
                            .outline()
                            .icon(IconName::File)
                            .label(t!("files.new_file"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_new_file(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("files-view-details")
                            .small()
                            .ghost()
                            .icon(IconName::GalleryVerticalEnd)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_view_mode(ViewMode::Details, cx);
                            })),
                    )
                    .child(
                        Button::new("files-view-grid")
                            .small()
                            .ghost()
                            .icon(IconName::LayoutDashboard)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_view_mode(ViewMode::Grid, cx);
                            })),
                    )
                    .child(
                        Button::new("files-delete-btn")
                            .small()
                            .outline()
                            .icon(IconName::Delete)
                            .disabled(selected_count == 0)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.perform_delete(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        DropdownButton::new("files-sort")
                            .small()
                            .outline()
                            .button(Button::new("files-sort-btn").label(sort_label))
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
            .child(self.file_list(cx))
            .child(
                h_flex()
                    .justify_between()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} {}, {} {}",
                        self.display_items.len(),
                        t!("files.status.items"),
                        selected_count,
                        t!("files.status.selected"),
                    ))
                    .child(t!("files.status.local")),
            )
    }
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
    };
    Rc::new(vec![size; count])
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
