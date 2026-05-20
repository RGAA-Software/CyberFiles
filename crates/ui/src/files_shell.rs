use gpui::{
    prelude::*, App, Context, Entity, FocusHandle, Focusable, ParentElement, Render, SharedString,
    Styled, Window, div,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    tab::{Tab, TabBar},
    v_flex, IconName, Sizable as _,
};

use crate::file_browser::FileBrowser;

struct FileTab {
    id: u64,
    title: SharedString,
    browser: Entity<FileBrowser>,
}

pub struct FilesShell {
    focus_handle: FocusHandle,
    tabs: Vec<FileTab>,
    active_tab: usize,
    next_tab_id: u64,
}

impl FilesShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let browser = cx.new(FileBrowser::new);
        Self {
            focus_handle: cx.focus_handle(),
            tabs: vec![FileTab {
                id: 0,
                title: "Files".into(),
                browser,
            }],
            active_tab: 0,
            next_tab_id: 1,
        }
    }

    fn active_browser(&self) -> Entity<FileBrowser> {
        self.tabs[self.active_tab].browser.clone()
    }

    fn add_tab(&mut self, cx: &mut Context<Self>) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let browser = cx.new(FileBrowser::new);
        self.tabs.push(FileTab {
            id,
            title: format!("Tab {}", id + 1).into(),
            browser,
        });
        self.active_tab = self.tabs.len() - 1;
    }

    fn close_tab(&mut self, index: usize) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(index);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if index < self.active_tab {
            self.active_tab -= 1;
        }
    }
}

impl Focusable for FilesShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FilesShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active_tab;
        let active_browser = self.active_browser();

        v_flex()
            .id("files-shell")
            .size_full()
            .min_h_0()
            .track_focus(&self.focus_handle)
            .child(
                TabBar::new("files-tab-bar")
                    .small()
                    .selected_index(active)
                    .last_empty_space(
                        h_flex()
                            .gap_1()
                            .pr_2()
                            .child(
                                Button::new("files-new-tab")
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::Plus)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.add_tab(cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                        let closable = self.tabs.len() > 1;
                        let mut tab_el = Tab::new().label(tab.title.clone());
                        if closable {
                            tab_el = tab_el.suffix(
                                Button::new(format!("files-tab-close-{}", tab.id))
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.close_tab(index);
                                        cx.notify();
                                    })),
                            );
                        }
                        tab_el
                    }))
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.active_tab = *ix;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(active_browser),
            )
    }
}
