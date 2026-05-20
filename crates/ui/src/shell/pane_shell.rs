use std::path::PathBuf;

use gpui::{prelude::*, *};

use crate::file_browser::FileBrowser;
use crate::home::HomePage;
use crate::settings_view::build_settings;
use crate::shell::navigation::NavigationTarget;

pub struct PaneShell {
    target: NavigationTarget,
    file_browser: Entity<FileBrowser>,
}

impl PaneShell {
    pub fn new(cx: &mut Context<Self>, target: NavigationTarget) -> Self {
        let initial_path = match &target {
            NavigationTarget::Path(path) => path.clone(),
            _ => cyberfiles_fs::home_navigation_path(),
        };
        Self {
            target,
            file_browser: cx.new(|cx| FileBrowser::for_shell(cx, initial_path)),
        }
    }

    pub fn target(&self) -> &NavigationTarget {
        &self.target
    }

    pub fn file_browser(&self) -> Entity<FileBrowser> {
        self.file_browser.clone()
    }

    pub fn navigate(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        self.target = target;
        if let NavigationTarget::Path(path) = &self.target {
            self.file_browser.update(cx, |browser, cx| {
                browser.open_directory_reset_history(path.clone());
                cx.notify();
            });
        }
        cx.notify();
    }
}

impl Render for PaneShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.target {
            NavigationTarget::Home => HomePage::render(cx).into_any_element(),
            NavigationTarget::Settings => div()
                .id("settings-page")
                .size_full()
                .min_h_0()
                .overflow_hidden()
                .child(build_settings(cx))
                .into_any_element(),
            NavigationTarget::Path(_) => self.file_browser.clone().into_any_element(),
        }
    }
}

impl PaneShell {
    pub fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.navigate(NavigationTarget::Path(path), cx);
    }
}
