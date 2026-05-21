use std::borrow::BorrowMut;
use std::path::PathBuf;

use cyberfiles_fs::{ClipboardOperation, FileClipboard};
use gpui::{App, AppContext, Entity, Global, Window};

use crate::main_page::MainPage;
use crate::shell::navigation::NavigationTarget;

/// Global handle so Home / pinned sidebar items can request tab navigation.
pub struct AppNavigation(Entity<MainPage>);

impl Global for AppNavigation {}

impl AppNavigation {
    pub fn set(main_page: Entity<MainPage>, cx: &mut App) {
        cx.set_global(Self(main_page));
    }

    pub fn navigate_to_path(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.navigate_to(NavigationTarget::Path(path), cx);
        });
    }

    pub fn open_path_in_new_tab(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.open_path_in_new_tab(path, cx);
        });
    }

    pub fn drop_paths_on_directory(
        dest: PathBuf,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut (impl AppContext + BorrowMut<App>),
    ) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| page.drop_paths_on_directory(dest, paths, window, cx));
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
            let _ = page.update(cx, |_, cx| cx.notify());
        });
    }

    pub fn navigate_breadcrumb(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let target = breadcrumb_navigation_target(&path);
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| page.navigate_to(target, cx));
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
    } else {
        NavigationTarget::Path(path.to_path_buf())
    }
}

/// In-app file clipboard for copy/cut/paste between folders.
pub struct AppFileClipboard(Option<FileClipboard>);

impl Global for AppFileClipboard {}

impl Default for AppFileClipboard {
    fn default() -> Self {
        Self(None)
    }
}

impl AppFileClipboard {
    pub fn take(cx: &mut (impl AppContext + BorrowMut<App>)) -> Option<FileClipboard> {
        cx.borrow_mut().global_mut::<Self>().0.take()
    }

    pub fn store(
        operation: ClipboardOperation,
        paths: Vec<PathBuf>,
        cx: &mut (impl AppContext + BorrowMut<App>),
    ) {
        cx.borrow_mut()
            .set_global(Self(Some(FileClipboard::new(operation, paths))));
    }

    pub fn set(clipboard: FileClipboard, cx: &mut (impl AppContext + BorrowMut<App>)) {
        cx.borrow_mut().set_global(Self(Some(clipboard)));
    }
}
