use std::borrow::BorrowMut;
use std::path::PathBuf;

use gpui::{App, AppContext, Entity, Global};

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
}
