use std::borrow::BorrowMut;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use cyberfiles_fs::{ClipboardOperation, FileClipboard};
use gpui::{App, AppContext, Entity, Global, SharedString, Window};

use crate::main_page::MainPage;
use crate::shell::navigation::NavigationTarget;

/// Active background file operation shown in the status bar (Files StatusCenter subset).
#[derive(Clone)]
pub struct ActiveTransfer {
    pub message: SharedString,
    completed: Arc<AtomicU32>,
    pub total: u32,
    cancel: Arc<AtomicBool>,
}

impl ActiveTransfer {
    pub fn new(message: SharedString, total: u32) -> Self {
        Self {
            message,
            completed: Arc::new(AtomicU32::new(0)),
            total: total.max(1),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn set_completed(&self, completed: u32) {
        self.completed.store(completed, Ordering::Relaxed);
    }

    pub fn completed(&self) -> u32 {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn fraction(&self) -> f32 {
        (self.completed() as f32 / self.total as f32).clamp(0., 1.)
    }
}

/// Status bar transfer progress and cancel (Files StatusCenter subset).
#[derive(Clone, Default)]
pub struct TransferStatusGlobal(pub Arc<RwLock<Option<ActiveTransfer>>>);

impl Global for TransferStatusGlobal {}

impl TransferStatusGlobal {
    pub fn init(cx: &mut App) {
        cx.set_global(Self(Arc::new(RwLock::new(None))));
    }

    pub fn begin(message: SharedString, total: u32, cx: &mut App) -> Arc<AtomicBool> {
        let job = ActiveTransfer::new(message, total);
        let cancel = job.cancel_flag();
        if let Some(global) = cx.try_global::<Self>() {
            if let Ok(mut guard) = global.0.write() {
                *guard = Some(job);
            }
        }
        Self::notify_main_page(cx);
        cancel
    }

    pub fn set_progress(completed: u32, cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else {
            return;
        };
        if let Ok(guard) = global.0.read() {
            if let Some(job) = guard.as_ref() {
                job.set_completed(completed);
            }
        }
        Self::notify_main_page(cx);
    }

    pub fn end(cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else {
            return;
        };
        if let Ok(mut guard) = global.0.write() {
            *guard = None;
        }
        Self::notify_main_page(cx);
    }

    pub fn request_cancel(cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else {
            return;
        };
        if let Ok(guard) = global.0.read() {
            if let Some(job) = guard.as_ref() {
                job.request_cancel();
            }
        }
        Self::notify_main_page(cx);
    }

    pub fn active(cx: &App) -> Option<ActiveTransfer> {
        cx.try_global::<Self>()
            .and_then(|g| g.0.read().ok().and_then(|j| j.clone()))
    }

    fn notify_main_page(cx: &mut App) {
        if let Some(nav) = cx.try_global::<AppNavigation>() {
            let page = nav.main_page();
            let _ = page.update(cx, |_, cx| cx.notify());
        }
    }
}

/// Global handle so Home / pinned sidebar items can request tab navigation.
pub struct AppNavigation(Entity<MainPage>);

impl Global for AppNavigation {}

impl AppNavigation {
    pub fn set(main_page: Entity<MainPage>, cx: &mut App) {
        cx.set_global(Self(main_page));
    }

    pub fn main_page(&self) -> Entity<MainPage> {
        self.0.clone()
    }

    pub fn navigate_to_path(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.navigate_to(NavigationTarget::Path(path), cx);
        });
    }

    pub fn navigate_to_file_tag(tag_name: String, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.navigate_to(NavigationTarget::FileTag(tag_name), cx);
        });
    }

    pub fn open_path_in_new_tab(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.open_path_in_new_tab(path, cx);
        });
    }

    /// Open a path in the secondary pane (enables dual pane if needed).
    pub fn open_path_in_secondary_pane(
        path: PathBuf,
        cx: &mut (impl AppContext + BorrowMut<App>),
    ) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| page.open_path_in_secondary_pane(path, cx));
    }

    pub fn focus_search(window: &mut Window, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| page.focus_search_input(window, cx));
    }

    /// Notify the shell so Omnibar breadcrumbs/path stay in sync with the active file browser.
    ///
    /// Deferred to avoid panics when called from nested updates (e.g. toolbar back inside `MainPage`).
    pub fn location_changed(cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        cx.borrow_mut().defer(move |cx| {
            let _ = page.update(cx, |page, cx| {
                // Folder open / back / up in the list must leave path-edit mode and show breadcrumbs.
                page.dismiss_omnibar_path_edit(cx);
                cx.notify();
            });
        });
    }

    pub fn pin_folder(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.pin_folder_path(path, cx);
            page.refresh_home_widgets(cx);
        });
    }

    pub fn unpin_folder(path_string: &str, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.unpin_folder_path(path_string, cx);
            page.refresh_home_widgets(cx);
        });
    }

    pub fn refresh_home_widgets(cx: &mut (impl AppContext + BorrowMut<App>)) {
        let nav = cx.borrow_mut().global::<Self>().0.clone();
        nav.update(cx, |page, cx| page.refresh_home_widgets(cx));
    }

}

pub fn breadcrumb_navigation_target(path: &std::path::Path) -> NavigationTarget {
    let key = path.to_string_lossy();
    if key.eq_ignore_ascii_case("home") {
        NavigationTarget::Home
    } else if key.eq_ignore_ascii_case("settings") {
        NavigationTarget::Settings
    } else if key.eq_ignore_ascii_case("recycle") {
        NavigationTarget::RecycleBin
    } else if let Some(name) = key.strip_prefix("tag:") {
        NavigationTarget::FileTag(name.to_string())
    } else {
        NavigationTarget::Path(path.to_path_buf())
    }
}

/// In-app file clipboard for copy/cut/paste between folders (Files ShelfPane data source).
pub struct AppFileClipboard(Option<FileClipboard>);

impl Global for AppFileClipboard {}

impl Default for AppFileClipboard {
    fn default() -> Self {
        Self(None)
    }
}

impl AppFileClipboard {
    pub fn peek(cx: &App) -> Option<FileClipboard> {
        cx.try_global::<Self>().and_then(|c| c.0.clone())
    }

    pub fn take(cx: &mut (impl AppContext + BorrowMut<App>)) -> Option<FileClipboard> {
        let taken = cx.borrow_mut().global_mut::<Self>().0.take();
        if taken.is_some() {
            Self::notify_main_page(cx);
        }
        taken
    }

    pub fn store(
        operation: ClipboardOperation,
        paths: Vec<PathBuf>,
        cx: &mut (impl AppContext + BorrowMut<App>),
    ) {
        cx.borrow_mut()
            .set_global(Self(Some(FileClipboard::new(operation, paths))));
        Self::notify_main_page(cx);
    }

    pub fn set(clipboard: FileClipboard, cx: &mut (impl AppContext + BorrowMut<App>)) {
        cx.borrow_mut().set_global(Self(Some(clipboard)));
        Self::notify_main_page(cx);
    }

    pub fn clear(cx: &mut (impl AppContext + BorrowMut<App>)) {
        if cx.borrow_mut().global_mut::<Self>().0.is_some() {
            cx.borrow_mut().global_mut::<Self>().0 = None;
            Self::notify_main_page(cx);
        }
    }

    pub fn has_items(cx: &mut (impl AppContext + BorrowMut<App>)) -> bool {
        cx.borrow_mut()
            .try_global::<Self>()
            .map(|clipboard| clipboard.0.is_some())
            .unwrap_or(false)
    }

    fn notify_main_page(cx: &mut (impl AppContext + BorrowMut<App>)) {
        let Some(page) = cx
            .borrow_mut()
            .try_global::<AppNavigation>()
            .map(|nav| nav.main_page())
        else {
            return;
        };
        let _ = page.update(cx, |_, cx| cx.notify());
    }
}
