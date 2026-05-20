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

    pub fn focus_search(window: &mut Window, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| page.focus_search_input(window, cx));
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
