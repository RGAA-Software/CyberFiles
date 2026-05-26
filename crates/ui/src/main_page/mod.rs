use std::path::PathBuf;
use cyberfiles_core::{
    load_config, record_path_history, save_config, ClosedTabSession, SessionPaneLayout, APP_NAME,
};

const MAX_CLOSED_TABS: usize = 12;
use cyberfiles_commands::{
    CopyItems, CutItems, FocusOmnibar, NavigateBack, NavigateForward, NavigateUp, PasteItems,
    ReopenClosedTab, SelectAll, FILE_BROWSER,
};
use cyberfiles_fs::home_navigation_path;
use gpui::{prelude::*, *};
use gpui_component::{
    input::InputState,
    v_flex,
};

use crate::file_ops::{spawn_file_transfer, FileTransferKind};
use crate::info_pane::InfoPane;
use crate::omnibar::OmnibarBreadcrumbCallbacks;
use crate::shell::app_menus;
use crate::shell::navigation::NavigationTarget;
use crate::shell::preferences::persist_window_bounds;
use crate::shell::ReopenClosedTabAt;
use crate::shell::{PaneShell, ShellPanes};
use crate::sidebar::SidebarSection;

mod omnibar;
mod render;
mod render_shell;
mod sidebar;

/// Matches Files `NavigationToolbar` height.
const NAV_TOOLBAR_HEIGHT: Pixels = px(48.);
/// Default medium `TabBar` height in the integrated title bar.
const TITLE_TAB_BAR_HEIGHT: Pixels = px(32.);
/// Fixed width per document tab in the title bar (label truncates inside).
const TITLE_TAB_WIDTH: Pixels = px(200.);
const TITLE_TAB_CLOSE_RIGHT_INSET: Pixels = px(5.);
/// Omnibar height (Files `AddressToolbarButtonStyle` uses 32px).
const OMNIBAR_BAR_HEIGHT: Pixels = px(32.);

struct TabEntry {
    id: u64,
    shell: Entity<ShellPanes>,
}

pub struct MainPage {
    focus_handle: FocusHandle,
    tabs: Vec<TabEntry>,
    active_tab: usize,
    next_tab_id: u64,
    tab_bar_scroll_handle: ScrollHandle,
    pending_tab_scroll_to_ix: Option<usize>,
    show_info_pane: bool,
    info_pane: Entity<InfoPane>,
    /// When true, show an editable path field instead of breadcrumb segments.
    omnibar_show_full_path: bool,
    omnibar_path_input: Option<Entity<InputState>>,
    _omnibar_path_subscription: Option<Subscription>,
    omnibar_breadcrumb_callbacks: Option<OmnibarBreadcrumbCallbacks>,
    omnibar_breadcrumb_width: f32,
    breadcrumb_drag_generation: u64,
    search_input: Option<Entity<InputState>>,
    _search_subscription: Option<Subscription>,
    sidebar_sections: Vec<SidebarSection>,
    sidebar_cache_key: u64,
    sidebar_cache_generation: u64,
    sidebar_cache_loading: bool,
    show_status_center: bool,
}

impl MainPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let config = load_config().unwrap_or_default();
        let show_info_pane = config.show_info_pane;
        let (tabs, active_tab, next_tab_id) = if config.session_tabs.is_empty() {
            let shell = cx.new(|cx| ShellPanes::new(cx, NavigationTarget::Home));
            (vec![TabEntry { id: 0, shell }], 0, 1)
        } else {
            let active = config
                .session_active_tab
                .min(config.session_tabs.len().saturating_sub(1));
            let mut restored = Vec::with_capacity(config.session_tabs.len());
            for (id, encoded) in config.session_tabs.iter().enumerate() {
                let target = Self::decode_session_target(encoded);
                let layout = config.session_pane_layouts.get(id).cloned();
                let shell = cx.new(|cx| {
                    let mut shell = ShellPanes::new(cx, target);
                    if let Some(ref layout) = layout {
                        shell.restore_layout(layout, Self::decode_session_target, cx);
                    }
                    shell
                });
                restored.push(TabEntry {
                    id: id as u64,
                    shell,
                });
            }
            let next_id = restored.len() as u64;
            (restored, active, next_id)
        };
        let this = Self {
            focus_handle: cx.focus_handle(),
            tabs,
            active_tab,
            next_tab_id,
            tab_bar_scroll_handle: ScrollHandle::new(),
            pending_tab_scroll_to_ix: Some(active_tab),
            show_info_pane,
            info_pane: cx.new(|_| InfoPane::new()),
            omnibar_show_full_path: false,
            omnibar_path_input: None,
            _omnibar_path_subscription: None,
            omnibar_breadcrumb_callbacks: None,
            omnibar_breadcrumb_width: 320.,
            breadcrumb_drag_generation: 0,
            search_input: None,
            _search_subscription: None,
            sidebar_sections: Vec::new(),
            sidebar_cache_key: 0,
            sidebar_cache_generation: 0,
            sidebar_cache_loading: false,
            show_status_center: false,
        };
        // Propagate initial show_info_pane to all file browsers.
        for tab in &this.tabs {
            let shell = tab.shell.clone();
            let panes = {
                let shell_ref = shell.read(cx);
                let mut panes = Vec::new();
                shell_ref.for_each_pane(|pane| {
                    panes.push(pane.clone());
                });
                panes
            };
            for pane in panes {
                let file_browser = pane.read(cx).file_browser();
                file_browser.update(cx, |browser, cx| {
                    browser.set_show_info_pane(show_info_pane, cx);
                });
            }
        }
        this
    }

    pub fn open_path_in_new_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        record_path_history(&path);
        self.add_tab(NavigationTarget::Path(path), cx);
    }

    pub fn open_path_in_secondary_pane(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        record_path_history(&path);
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| {
            if !shell.dual_pane() {
                shell.toggle_dual_pane(cx);
            }
            shell.secondary().update(cx, |pane, cx| {
                pane.open_path(path, cx);
            });
            shell.set_active(crate::shell::PaneSide::Secondary, cx);
        });
        cx.notify();
    }

    pub fn drop_paths_on_directory(
        &mut self,
        dest: PathBuf,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() || !dest.is_dir() {
            return;
        }
        if paths.iter().all(|p| p.parent() == Some(dest.as_path())) {
            return;
        }
        let copy = window.modifiers().control;
        let kind = if copy {
            FileTransferKind::Copy
        } else {
            FileTransferKind::Move
        };
        let pane = self.active_pane(cx);
        let browser = pane.read(cx).file_browser().clone();
        browser.update(cx, |_, cx| {
            spawn_file_transfer(browser.clone(), window, cx, kind, paths, dest);
        });
        cx.notify();
    }

    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        app_menus::init(APP_NAME, cx);
        crate::app_state::TransferStatusGlobal::init(cx);
        let page = cx.new(|cx| Self::new(cx));
        crate::app_state::AppNavigation::set(page.clone(), cx);
        page
    }

    fn encode_session_target(target: &NavigationTarget, current_path: Option<&PathBuf>) -> String {
        match target {
            NavigationTarget::Home => "home".into(),
            NavigationTarget::RecycleBin => "recycle".into(),
            NavigationTarget::Settings => "settings".into(),
            NavigationTarget::FileTag(name) => format!("tag:{name}"),
            NavigationTarget::Path(_) => current_path
                .cloned()
                .unwrap_or_else(home_navigation_path)
                .to_string_lossy()
                .into_owned(),
        }
    }

    fn decode_session_target(value: &str) -> NavigationTarget {
        NavigationTarget::decode_session_tab(value)
    }

    fn capture_tab_session(&self, index: usize, cx: &App) -> ClosedTabSession {
        let shell = self.tabs[index].shell.read(cx);
        let pane = shell.active_pane().read(cx);
        let current_path = match pane.target() {
            NavigationTarget::Path(_) => {
                Some(pane.file_browser().read(cx).current_directory().clone())
            }
            _ => None,
        };
        ClosedTabSession {
            tab: Self::encode_session_target(pane.target(), current_path.as_ref()),
            pane_layout: Self::capture_shell_layout(shell, cx),
        }
    }

    fn record_closed_tab(&self, session: ClosedTabSession) {
        let mut config = load_config().unwrap_or_default();
        config
            .session_closed_tabs
            .retain(|closed| closed.tab != session.tab);
        config.session_closed_tabs.insert(0, session);
        config.session_closed_tabs.truncate(MAX_CLOSED_TABS);
        let _ = save_config(&config);
    }

    pub fn reopen_closed_tab(&mut self, cx: &mut Context<Self>) {
        self.reopen_closed_tab_at(0, cx);
    }

    pub fn reopen_closed_tab_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        if index >= config.session_closed_tabs.len() {
            return;
        }
        let closed = config.session_closed_tabs.remove(index);
        let _ = save_config(&config);

        let target = Self::decode_session_target(&closed.tab);
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let layout = closed.pane_layout;
        let shell = cx.new(|cx| {
            let mut shell = ShellPanes::new(cx, target);
            shell.restore_layout(&layout, Self::decode_session_target, cx);
            shell
        });
        self.tabs.push(TabEntry { id, shell });
        self.active_tab = self.tabs.len() - 1;
        self.pending_tab_scroll_to_ix = Some(self.active_tab);
        self.persist_session(cx);
        app_menus::reload(cx);
        cx.notify();
    }

    fn capture_shell_layout(shell: &ShellPanes, cx: &App) -> SessionPaneLayout {
        let secondary_tab = if shell.dual_pane() {
            let pane = shell.secondary().read(cx);
            let current_path = match pane.target() {
                NavigationTarget::Path(_) => {
                    Some(pane.file_browser().read(cx).current_directory().clone())
                }
                _ => None,
            };
            Self::encode_session_target(pane.target(), current_path.as_ref())
        } else {
            String::new()
        };
        let active_side = match shell.active_side() {
            crate::shell::PaneSide::Secondary => "secondary",
            crate::shell::PaneSide::Primary => "primary",
        };
        SessionPaneLayout {
            dual_pane: shell.dual_pane(),
            active_side: active_side.into(),
            secondary_tab,
        }
    }

    pub fn persist_session(&mut self, cx: &mut Context<Self>) {
        let tabs: Vec<String> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let pane = self.tabs[index].shell.read(cx).active_pane().read(cx);
                let current_path = match pane.target() {
                    NavigationTarget::Path(_) => {
                        Some(pane.file_browser().read(cx).current_directory().clone())
                    }
                    _ => None,
                };
                Self::encode_session_target(pane.target(), current_path.as_ref())
            })
            .collect();
        let layouts: Vec<SessionPaneLayout> = self
            .tabs
            .iter()
            .map(|entry| Self::capture_shell_layout(&entry.shell.read(cx), cx))
            .collect();
        let mut config = load_config().unwrap_or_default();
        config.session_tabs = tabs;
        config.session_active_tab = self.active_tab;
        config.session_pane_layouts = layouts;
        let _ = save_config(&config);
    }

    fn active_shell(&self) -> Entity<ShellPanes> {
        self.tabs[self.active_tab].shell.clone()
    }

    fn active_pane(&self, cx: &App) -> Entity<PaneShell> {
        self.active_shell().read(cx).active_pane()
    }

    fn active_file_browser(&self, cx: &App) -> Entity<crate::file_browser::FileBrowser> {
        self.active_pane(cx).read(cx).file_browser()
    }

    fn file_navigation_active(&self, cx: &App) -> bool {
        matches!(
            self.active_pane(cx).read(cx).target(),
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin | NavigationTarget::FileTag(_)
        )
    }

    pub fn navigate_to(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        if let NavigationTarget::Path(ref path) = target {
            record_path_history(path);
        }
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| {
            shell.navigate_active(target, cx);
        });
        self.omnibar_show_full_path = false;
        self.persist_session(cx);
        cx.notify();
    }

    fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| shell.toggle_dual_pane(cx));
        self.persist_session(cx);
        cx.notify();
    }

    pub fn active_navigation_target(&self, cx: &App) -> NavigationTarget {
        self.active_pane(cx).read(cx).current_navigation_target(cx)
    }

    fn toggle_info_pane(&mut self, cx: &mut Context<Self>) {
        self.show_info_pane = !self.show_info_pane;
        let mut config = load_config().unwrap_or_default();
        config.show_info_pane = self.show_info_pane;
        let _ = save_config(&config);
        // Notify all file browsers so they can recalculate grid/card column counts.
        for tab in &self.tabs {
            let shell = tab.shell.clone();
            let panes = {
                let shell_ref = shell.read(cx);
                let mut panes = Vec::new();
                shell_ref.for_each_pane(|pane| {
                    panes.push(pane.clone());
                });
                panes
            };
            for pane in panes {
                let file_browser = pane.read(cx).file_browser();
                file_browser.update(cx, |browser, cx| {
                    browser.set_show_info_pane(self.show_info_pane, cx);
                });
            }
        }
        cx.notify();
    }

    fn info_selection(&self, cx: &App) -> Option<cyberfiles_fs::FileItem> {
        let pane = self.active_pane(cx);
        if !matches!(
            pane.read(cx).target(),
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin
        ) {
            return None;
        }
        pane.read(cx)
            .file_browser()
            .read(cx)
            .primary_selected_item()
            .cloned()
    }

    fn add_tab(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let shell = cx.new(|cx| ShellPanes::new(cx, target));
        self.tabs.push(TabEntry { id, shell });
        self.active_tab = self.tabs.len() - 1;
        self.pending_tab_scroll_to_ix = Some(self.active_tab);
        self.persist_session(cx);
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            persist_window_bounds(cx);
            cyberfiles_core::flush_config();
            cx.quit();
            return;
        }
        let closed = self.capture_tab_session(index, cx);
        self.record_closed_tab(closed);
        app_menus::reload(cx);
        self.tabs.remove(index);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if index < self.active_tab {
            self.active_tab -= 1;
        }
        self.pending_tab_scroll_to_ix = Some(self.active_tab);
        self.persist_session(cx);
        cx.notify();
    }

    fn tab_title(&self, index: usize, cx: &App) -> SharedString {
        let pane = self.tabs[index].shell.read(cx).active_pane().read(cx);
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

}

impl Focusable for MainPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_sidebar_cache(cx);
        let active_shell = self.active_shell();
        let show_info_pane = self.show_info_pane;
        let file_navigation_active = self.file_navigation_active(cx);
        let info_item = self.info_selection(cx);
        self.info_pane
            .update(cx, |pane, _| pane.set_item(info_item));

        v_flex()
            .id("main-page")
            .size_full()
            .min_h_0()
            .min_w_0()
            .track_focus(&self.focus_handle)
            .when(file_navigation_active, |page| {
                page.key_context(FILE_BROWSER)
            })
            .on_action(cx.listener(|this, _: &FocusOmnibar, window, cx| {
                this.focus_search_input(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NavigateUp, _, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                this.active_file_browser(cx)
                    .update(cx, |browser, cx| browser.go_up(cx));
            }))
            .on_action(cx.listener(|this, _: &NavigateBack, _, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                this.active_file_browser(cx)
                    .update(cx, |browser, cx| browser.go_back(cx));
            }))
            .on_action(cx.listener(|this, _: &NavigateForward, _, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                this.active_file_browser(cx)
                    .update(cx, |browser, cx| browser.go_forward(cx));
            }))
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                if window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                let active_browser = this.active_file_browser(cx);
                active_browser.update(cx, |browser, cx| {
                    browser.select_all();
                    cx.notify();
                });
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &CopyItems, window, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                if window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                let active_browser = this.active_file_browser(cx);
                active_browser.update(cx, |browser, cx| {
                    browser.copy_items(cx);
                    cx.notify();
                });
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &CutItems, window, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                if window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                let active_browser = this.active_file_browser(cx);
                active_browser.update(cx, |browser, cx| {
                    browser.cut_items(cx);
                    cx.notify();
                });
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &PasteItems, window, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                if window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                let active_browser = this.active_file_browser(cx);
                active_browser.update(cx, |browser, cx| {
                    browser.paste_items(window, cx);
                });
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &ReopenClosedTab, _, cx| {
                this.reopen_closed_tab(cx);
            }))
            .on_action(cx.listener(|this, action: &ReopenClosedTabAt, _, cx| {
                this.reopen_closed_tab_at(action.index, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    if this.show_status_center {
                        this.show_status_center = false;
                        cx.notify();
                    } else {
                        this.dismiss_omnibar_path_edit(cx);
                    }
                }
            }))
            .child(self.render_title_bar(window, cx))
            .child(self.render_navigation_toolbar(window, cx))
            .child(
                div()
                    .id("main-body")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.render_shell_layout_row(window, active_shell, show_info_pane, cx)),
            )
    }
}
