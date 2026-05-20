use cyberfiles_fs::{home_navigation_path, list_drives};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    resizable::{h_resizable, resizable_panel},
    sidebar::{
        Sidebar, SidebarGroup, SidebarHeader, SidebarItem, SidebarMenu, SidebarMenuItem,
    },
    tab::{Tab, TabBar},
    v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use rust_i18n::t;

use crate::shell::navigation::NavigationTarget;
use crate::shell::PaneShell;
use cyberfiles_core::APP_NAME;

struct TabEntry {
    id: u64,
    pane: Entity<PaneShell>,
}

pub struct MainPage {
    focus_handle: FocusHandle,
    tabs: Vec<TabEntry>,
    active_tab: usize,
    next_tab_id: u64,
}

impl MainPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let pane = cx.new(|cx| PaneShell::new(cx, NavigationTarget::Home));
        Self {
            focus_handle: cx.focus_handle(),
            tabs: vec![TabEntry { id: 0, pane }],
            active_tab: 0,
            next_tab_id: 1,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(cx))
    }

    fn active_pane(&self) -> Entity<PaneShell> {
        self.tabs[self.active_tab].pane.clone()
    }

    fn navigate_active(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        let pane = self.active_pane();
        pane.update(cx, |shell, cx| {
            shell.navigate(target, cx);
        });
        cx.notify();
    }

    fn add_tab(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let pane = cx.new(|cx| PaneShell::new(cx, target));
        self.tabs.push(TabEntry { id, pane });
        self.active_tab = self.tabs.len() - 1;
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(index);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if index < self.active_tab {
            self.active_tab -= 1;
        }
        cx.notify();
    }

    fn tab_title(&self, index: usize, cx: &App) -> SharedString {
        let pane = self.tabs[index].pane.read(cx);
        match pane.target() {
            NavigationTarget::Path(_) => {
                let path = pane.file_browser().read(cx).current_directory();
                SharedString::from(
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string()),
                )
            }
            target => target.tab_title(),
        }
    }

    fn render_navigation_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.active_pane();
        let target = pane.read(cx).target().clone();
        let path_label = target.toolbar_path_label();

        let browser = pane.read(cx).file_browser();
        let (can_back, can_forward, can_up) = if matches!(target, NavigationTarget::Path(_)) {
            let b = browser.read(cx);
            (b.can_go_back(), b.can_go_forward(), b.can_go_up())
        } else {
            (false, false, false)
        };
        let show_file_ops = matches!(target, NavigationTarget::Path(_));

        h_flex()
            .id("navigation-toolbar")
            .gap_2()
            .px_3()
            .py_2()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("nav-back")
                    .small()
                    .ghost()
                    .icon(IconName::ArrowLeft)
                    .disabled(!can_back)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let pane = this.active_pane();
                        pane.update(cx, |shell, cx| {
                            if let NavigationTarget::Path(_) = shell.target() {
                                shell.file_browser().update(cx, |b, cx| {
                                    b.go_back();
                                    cx.notify();
                                });
                            }
                        });
                        cx.notify();
                    })),
            )
            .child(
                Button::new("nav-forward")
                    .small()
                    .ghost()
                    .icon(IconName::ArrowRight)
                    .disabled(!can_forward)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let pane = this.active_pane();
                        pane.update(cx, |shell, cx| {
                            shell.file_browser().update(cx, |b, cx| {
                                b.go_forward();
                                cx.notify();
                            });
                        });
                        cx.notify();
                    })),
            )
            .child(
                Button::new("nav-up")
                    .small()
                    .ghost()
                    .icon(IconName::ArrowUp)
                    .disabled(!can_up)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let pane = this.active_pane();
                        pane.update(cx, |shell, cx| {
                            shell.file_browser().update(cx, |b, cx| {
                                b.go_up();
                                cx.notify();
                            });
                        });
                        cx.notify();
                    })),
            )
            .child(
                Button::new("nav-refresh")
                    .small()
                    .ghost()
                    .icon(IconName::Redo2)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let pane = this.active_pane();
                        pane.update(cx, |shell, cx| {
                            shell.file_browser().update(cx, |b, cx| {
                                b.reload();
                                cx.notify();
                            });
                        });
                        cx.notify();
                    })),
            )
            .when(show_file_ops, |bar| {
                bar.child(
                    Button::new("nav-new-folder")
                        .small()
                        .outline()
                        .icon(IconName::Folder)
                        .label(t!("files.new_folder"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            let pane = this.active_pane();
                            pane.update(cx, |shell, cx| {
                                shell.file_browser().update(cx, |b, cx| {
                                    b.create_new_folder(window, cx);
                                });
                            });
                            cx.notify();
                        })),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_3()
                    .py_1()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(path_label),
            )
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.active_pane();
        let target = pane.read(cx).target().clone();

        let (items, selected, hint) = match target {
            NavigationTarget::Path(_) => {
                let b = pane.read(cx).file_browser().read(cx);
                (
                    b.item_count(),
                    b.selected_count(),
                    t!("files.status.local").to_string(),
                )
            }
            NavigationTarget::Home => (0, 0, t!("main.status.home").to_string()),
            NavigationTarget::Settings => (0, 0, t!("main.status.settings").to_string()),
        };

        h_flex()
            .id("status-bar")
            .h_8()
            .px_3()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(cx.theme().border)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!(
                "{} {}, {} {}",
                items,
                t!("files.status.items"),
                selected,
                t!("files.status.selected")
            ))
            .child(hint)
    }

    fn render_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drives = list_drives();

        Sidebar::new("files-sidebar")
            .w(relative(1.))
            .border_0()
            .header(
                SidebarHeader::new().child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .rounded(cx.theme().radius_lg)
                                .bg(cx.theme().primary)
                                .text_color(cx.theme().primary_foreground)
                                .size_8()
                                .flex_shrink_0()
                                .child(Icon::new(IconName::GalleryVerticalEnd)),
                        )
                        .child(
                            v_flex()
                                .gap_0()
                                .text_sm()
                                .child(APP_NAME)
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(t!("sidebar.workspace")),
                                ),
                        ),
                ),
            )
            .child(
                SidebarGroup::new(t!("sidebar.section.main"))
                    .child(
                        SidebarMenu::new().w_full().child(
                            SidebarMenuItem::new(t!("nav.home"))
                                .icon(IconName::LayoutDashboard)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_active(NavigationTarget::Home, cx);
                                })),
                        ),
                    ),
            )
            .child(
                SidebarGroup::new(t!("sidebar.section.pinned"))
                    .child(
                        SidebarMenu::new().w_full().child(
                            SidebarMenuItem::new(t!("sidebar.pinned.placeholder"))
                                .icon(IconName::Star)
                                .disable(true),
                        ),
                    ),
            )
            .child(
                SidebarGroup::new(t!("sidebar.section.drives"))
                    .child(
                        SidebarMenu::new().w_full().children(drives.iter().map(|drive| {
                            let path = drive.path.clone();
                            SidebarMenuItem::new(drive.label.clone())
                                .icon(IconName::HardDrive)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.navigate_active(NavigationTarget::Path(path.clone()), cx);
                                }))
                        })),
                    ),
            )
            .child(
                SidebarGroup::new(t!("sidebar.section.network"))
                    .child(
                        SidebarMenu::new().w_full().child(
                            SidebarMenuItem::new(t!("sidebar.network.placeholder"))
                                .icon(IconName::Globe)
                                .disable(true),
                        ),
                    ),
            )
            .footer(
                v_flex().flex_1().w_full().min_w_0().child(
                    SidebarMenu::new()
                        .w_full()
                        .child(
                            SidebarMenuItem::new(t!("nav.settings"))
                                .icon(IconName::Settings2)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_active(NavigationTarget::Settings, cx);
                                })),
                        )
                        .render("sidebar-settings", window, cx),
                ),
            )
    }
}

impl Focusable for MainPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active_tab;
        let active_pane = self.active_pane();

        v_flex()
            .id("main-page")
            .size_full()
            .min_h_0()
            .track_focus(&self.focus_handle)
            .child(
                TabBar::new("main-tab-bar")
                    .small()
                    .selected_index(active)
                    .last_empty_space(
                        h_flex()
                            .gap_1()
                            .pr_2()
                            .child(
                                Button::new("main-new-tab")
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::Plus)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.add_tab(
                                            NavigationTarget::Path(home_navigation_path()),
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                        let title = self.tab_title(index, cx);
                        let closable = self.tabs.len() > 1;
                        let mut tab_el = Tab::new().label(title);
                        if closable {
                            tab_el = tab_el.suffix(
                                Button::new(format!("main-tab-close-{}", tab.id))
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.close_tab(index, cx);
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
                    .child(
                h_resizable("main-layout")
                    .child(
                        resizable_panel()
                            .size(px(240.))
                            .size_range(px(200.)..px(360.))
                            .child(self.render_sidebar(window, cx)),
                    )
                    .child(
                        resizable_panel()
                            .flex_1()
                            .child(
                                v_flex()
                                    .size_full()
                                    .min_h_0()
                                    .child(self.render_navigation_toolbar(cx))
                                    .child(
                                        div()
                                            .id("main-content")
                                            .flex_1()
                                            .min_h_0()
                                            .overflow_hidden()
                                            .child(active_pane),
                                    )
                                    .child(self.render_status_bar(cx)),
                            ),
                    ),
                    ),
            )
    }
}
