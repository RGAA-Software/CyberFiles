use std::path::PathBuf;
use std::rc::Rc;

use cyberfiles_core::{
    load_config, record_path_history, save_config, ClosedTabSession, SessionPaneLayout, APP_NAME,
};

const MAX_CLOSED_TABS: usize = 12;
use cyberfiles_commands::{
    FocusOmnibar, NavigateBack, NavigateForward, NavigateUp, PasteItems, ReopenClosedTab,
    FILE_BROWSER,
};
use cyberfiles_fs::{
    breadcrumb_root_menu_sections, home_navigation_path, list_drives, path_breadcrumbs,
    PathBreadcrumb,
};
use cyberfiles_platform_windows::list_shell_quick_access_folders;
use gpui::{prelude::*, *};
use gpui_component::{
    badge::Badge,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,

    resizable::{h_resizable, resizable_panel},
    v_flex, ActiveTheme as _, Disableable as _, ElementExt as _, IconName, Sizable as _, Size,
    ThemeMode, WindowExt as _,
};
use rust_i18n::t;

use crate::app_state::{
    breadcrumb_navigation_target, AppFileClipboard, AppNavigation,
    TransferStatusGlobal,
};
use crate::file_ops::{spawn_file_transfer, FileTransferKind};
use crate::icons::{compact_icon, pin_icon, toolbar_icon};
use crate::info_pane::InfoPane;
use crate::omnibar::{OmnibarBreadcrumbCallbacks, BREADCRUMB_DRAG_HOVER_OPEN_MS};
use crate::shell::app_menus;
use crate::shell::navigation::NavigationTarget;
use crate::shell::preferences::{apply_theme_mode, persist_window_bounds};
use crate::shell::ReopenClosedTabAt;
use crate::shell::{PaneShell, ShellPanes};
use crate::sidebar::{render_sidebar, sidebar_cache_key, SidebarSection};
use crate::tab::{Tab, TabBar};
use crate::title_bar::TitleBar;
use crate::toolbar_button::toolbar_icon_button;

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
        let mut this = Self {
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

    /// Rebuild sidebar section lists when settings or pins change (async when cache exists).
    pub fn refresh_sidebar_cache(&mut self, cx: &mut Context<Self>) {
        self.sidebar_cache_key = 0;
        self.ensure_sidebar_cache(cx);
    }

    /// Reload file browsers that are browsing `tag_name` (all tabs / dual panes).
    pub fn reload_file_tag_browsers(&mut self, tag_name: &str, cx: &mut Context<Self>) {
        for tab in &self.tabs {
            let shell = tab.shell.clone();
            shell.update(cx, |shell, cx| {
                shell.for_each_pane(|pane| {
                    pane.update(cx, |pane, cx| {
                        if matches!(
                            pane.target(),
                            NavigationTarget::FileTag(name) if name == tag_name
                        ) {
                            pane.file_browser().update(cx, |browser, cx| {
                                browser.reload();
                                cx.notify();
                            });
                        }
                    });
                });
            });
        }
    }

    fn ensure_sidebar_cache(&mut self, cx: &mut Context<Self>) {
        let config = load_config().unwrap_or_default();
        let key = sidebar_cache_key(&config);
        if self.sidebar_cache_key == key && !self.sidebar_sections.is_empty() {
            return;
        }
        if self.sidebar_cache_loading {
            return;
        }
        if self.sidebar_sections.is_empty() {
            self.sidebar_sections = crate::sidebar::build_sidebar_sections_cached(&config);
            self.sidebar_cache_key = key;
            return;
        }
        self.sidebar_cache_loading = true;
        self.sidebar_cache_generation = self.sidebar_cache_generation.wrapping_add(1);
        let generation = self.sidebar_cache_generation;
        cx.spawn(async move |page, cx| {
            let sections = cx
                .background_spawn(async move {
                    let config = load_config().unwrap_or_default();
                    crate::sidebar::build_sidebar_sections_cached(&config)
                })
                .await;
            let key = load_config().map(|c| sidebar_cache_key(&c)).unwrap_or(0);
            let _ = page.update(cx, |page, cx| {
                page.sidebar_cache_loading = false;
                if page.sidebar_cache_generation != generation {
                    return;
                }
                page.sidebar_sections = sections;
                page.sidebar_cache_key = key;
                cx.notify();
            });
        })
        .detach();
    }

    fn ensure_omnibar_breadcrumb_callbacks(&mut self, cx: &mut Context<Self>) {
        if self.omnibar_breadcrumb_callbacks.is_some() {
            return;
        }
        let page = cx.entity();
        let on_navigate = Rc::new(move |path: PathBuf, _: &mut Window, cx: &mut App| {
            let _ = page.update(cx, |page, cx| {
                page.navigate_to(breadcrumb_navigation_target(&path), cx);
            });
        });
        let page_tab = cx.entity();
        let on_navigate_new_tab = Rc::new(move |path: PathBuf, _: &mut Window, cx: &mut App| {
            let _ = page_tab.update(cx, |page, cx| page.open_path_in_new_tab(path, cx));
        });
        let page_home = cx.entity();
        let on_home = Rc::new(move |_: &mut Window, cx: &mut App| {
            let _ = page_home.update(cx, |page, cx| {
                page.navigate_to(NavigationTarget::Home, cx);
            });
        });
        let page_drop = cx.entity();
        let on_drop_paths = Rc::new(
            move |dest: PathBuf, paths: Vec<PathBuf>, window: &mut Window, cx: &mut App| {
                let _ = page_drop.update(cx, |page, cx| {
                    page.drop_paths_on_directory(dest, paths, window, cx);
                });
            },
        );
        let page_hover = cx.entity();
        let on_drag_hover = Rc::new(move |path: PathBuf, _: &mut Window, cx: &mut App| {
            let _ = page_hover.update(cx, |page, cx| {
                page.schedule_breadcrumb_drag_preview(path, cx);
            });
        });
        let page_path_bar = cx.entity();
        let on_show_full_path = Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = page_path_bar.update(cx, |page, cx| {
                page.enter_omnibar_path_edit(window, cx);
            });
        });
        let root_menu = Rc::new(|| {
            let quick_access: Vec<(String, PathBuf)> = {
                #[cfg(windows)]
                {
                    list_shell_quick_access_folders()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|e| (e.display_name, e.path))
                        .collect()
                }
                #[cfg(not(windows))]
                {
                    pinned_folder_paths()
                        .into_iter()
                        .map(|p| {
                            let label = p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| p.to_string_lossy().to_string());
                            (label, p)
                        })
                        .collect()
                }
            };
            let drive_entries: Vec<(String, PathBuf)> = list_drives()
                .into_iter()
                .map(|d| (d.label, d.path))
                .collect();
            breadcrumb_root_menu_sections(
                quick_access,
                drive_entries,
                Some(t!("omnibar.breadcrumb.quick_access").to_string()),
                Some(t!("omnibar.breadcrumb.drives").to_string()),
            )
        });
        self.omnibar_breadcrumb_callbacks = Some(OmnibarBreadcrumbCallbacks::new(
            true,
            root_menu,
            on_navigate,
            on_navigate_new_tab,
            on_home,
            on_drop_paths,
            on_drag_hover,
            on_show_full_path,
        ));
    }

    fn omnibar_working_directory(&self, cx: &App) -> Option<PathBuf> {
        let pane = self.active_pane(cx);
        if matches!(pane.read(cx).target(), NavigationTarget::Path(_)) {
            Some(
                pane.read(cx)
                    .file_browser()
                    .read(cx)
                    .current_directory()
                    .to_path_buf(),
            )
        } else {
            None
        }
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

    pub fn schedule_breadcrumb_drag_preview(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.omnibar_working_directory(cx).as_ref() == Some(&path) {
            return;
        }
        self.breadcrumb_drag_generation = self.breadcrumb_drag_generation.wrapping_add(1);
        let generation = self.breadcrumb_drag_generation;
        let target = breadcrumb_navigation_target(&path);
        cx.spawn(async move |page, cx| {
            cx.background_spawn(async move {
                std::thread::sleep(std::time::Duration::from_millis(
                    BREADCRUMB_DRAG_HOVER_OPEN_MS,
                ));
            })
            .await;
            let _ = page.update(cx, |page, cx| {
                if page.breadcrumb_drag_generation != generation {
                    return;
                }
                page.navigate_to(target, cx);
            });
        })
        .detach();
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

    fn ensure_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.search_input.clone() {
            return input;
        }

        let input = cx.new(|cx| InputState::new(window, cx).placeholder(t!("search.placeholder")));
        self._search_subscription = Some(cx.subscribe(
            &input,
            move |page, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    page.apply_search_from_input(cx);
                }
            },
        ));
        self.search_input = Some(input.clone());
        input
    }

    pub fn focus_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.ensure_search_input(window, cx);
        input.update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    fn apply_search_from_input(&mut self, cx: &mut Context<Self>) {
        let query = self
            .search_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let pane = self.active_pane(cx);
        pane.update(cx, |shell, cx| {
            if matches!(shell.target(), NavigationTarget::Path(_)) {
                shell.file_browser().update(cx, |browser, cx| {
                    browser.set_search_query(query, cx);
                });
            }
        });
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

    pub fn omnibar_path_edit_active(&self) -> bool {
        self.omnibar_show_full_path
    }

    pub fn dismiss_omnibar_path_edit(&mut self, cx: &mut Context<Self>) {
        if !self.omnibar_show_full_path {
            return;
        }
        self.omnibar_show_full_path = false;
        cx.notify();
    }

    fn ensure_omnibar_path_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.omnibar_path_input.clone() {
            return input;
        }

        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("nav.path.placeholder")));
        self._omnibar_path_subscription = Some(cx.subscribe(
            &input,
            move |page, _, event: &InputEvent, cx| match event {
                InputEvent::PressEnter { .. } => page.submit_omnibar_path(cx),
                InputEvent::Blur => page.dismiss_omnibar_path_edit(cx),
                _ => {}
            },
        ));
        self.omnibar_path_input = Some(input.clone());
        input
    }

    pub fn enter_omnibar_path_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.omnibar_show_full_path = true;
        let text = self.omnibar_full_path_text(cx);
        let input = self.ensure_omnibar_path_input(window, cx);
        input.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    fn submit_omnibar_path(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.omnibar_path_input.clone() else {
            return;
        };
        let text = input.read(cx).value().to_string();
        if let Some(target) = Self::resolve_path_submit(&text) {
            if let NavigationTarget::Path(ref path) = target {
                record_path_history(path);
            }
            self.omnibar_show_full_path = false;
            self.navigate_to(target, cx);
        }
    }

    fn resolve_path_submit(text: &str) -> Option<NavigationTarget> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.eq_ignore_ascii_case("home") {
            return Some(NavigationTarget::Home);
        }
        if trimmed.eq_ignore_ascii_case("settings") {
            return Some(NavigationTarget::Settings);
        }
        if trimmed.eq_ignore_ascii_case("recycle bin") || trimmed.eq_ignore_ascii_case("recycle") {
            return Some(NavigationTarget::RecycleBin);
        }

        let path = PathBuf::from(trimmed);
        if path.is_dir() {
            return Some(NavigationTarget::Path(path));
        }
        if path.is_file() {
            return path
                .parent()
                .map(|parent| NavigationTarget::Path(parent.to_path_buf()));
        }
        None
    }

    fn omnibar_full_path_text(&self, cx: &App) -> String {
        let pane = self.active_pane(cx);
        match pane.read(cx).current_navigation_target(cx) {
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin => pane
                .read(cx)
                .file_browser()
                .read(cx)
                .current_directory()
                .to_string_lossy()
                .to_string(),
            target => target.toolbar_path_label(),
        }
    }

    fn omnibar_breadcrumbs(&self, cx: &App) -> Vec<PathBreadcrumb> {
        let pane = self.active_pane(cx);
        let target = pane.read(cx).current_navigation_target(cx);
        match target {
            NavigationTarget::Path(_) => {
                let dir = pane
                    .read(cx)
                    .file_browser()
                    .read(cx)
                    .current_directory()
                    .clone();
                path_breadcrumbs(&dir)
            }
            NavigationTarget::Home => Vec::new(),
            NavigationTarget::Settings => vec![PathBreadcrumb {
                label: t!("nav.settings").to_string(),
                path: PathBuf::from("settings"),
            }],
            NavigationTarget::RecycleBin => vec![PathBreadcrumb {
                label: t!("nav.recycle_bin").to_string(),
                path: PathBuf::from("recycle"),
            }],
            NavigationTarget::FileTag(name) => vec![PathBreadcrumb {
                label: name.clone(),
                path: PathBuf::from(format!("tag:{name}")),
            }],
        }
    }

    fn render_omnibar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let show_breadcrumbs = !self.omnibar_show_full_path;
        let path_input = if self.omnibar_show_full_path {
            Some(self.ensure_omnibar_path_input(window, cx))
        } else {
            None
        };
        self.ensure_omnibar_breadcrumb_callbacks(cx);
        let breadcrumbs = self.omnibar_breadcrumbs(cx);
        let working_directory = self.omnibar_working_directory(cx);
        let read_options = *self
            .active_pane(cx)
            .read(cx)
            .file_browser()
            .read(cx)
            .read_options();
        let breadcrumb_width = self.omnibar_breadcrumb_width.max(1.);
        let breadcrumb_callbacks = self
            .omnibar_breadcrumb_callbacks
            .as_ref()
            .expect("breadcrumb callbacks");
        let breadcrumb_bar = breadcrumb_callbacks.breadcrumb_bar(
            breadcrumbs,
            breadcrumb_width,
            read_options,
            working_directory,
        );

        h_flex()
            .id("omnibar-bar")
            .w_full()
            .h(OMNIBAR_BAR_HEIGHT)
            .min_h(OMNIBAR_BAR_HEIGHT)
            .max_h(OMNIBAR_BAR_HEIGHT)
            .min_w_0()
            .items_center()
            .px_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .relative()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when(show_breadcrumbs, |bar| {
                bar.child({
                    let page = cx.entity();
                    h_flex()
                        .id("omnibar-breadcrumb-host")
                        .w_full()
                        .min_w_0()
                        .flex_1()
                        .overflow_x_hidden()
                        .items_center()
                        .on_prepaint(move |bounds, _, cx| {
                            let w = f32::from(bounds.size.width);
                            if w < 1.0 {
                                return;
                            }
                            let _ = page.update(cx, |page, cx| {
                                if (page.omnibar_breadcrumb_width - w).abs() > 1.5 {
                                    page.omnibar_breadcrumb_width = w;
                                    cx.notify();
                                }
                            });
                        })
                        .child(breadcrumb_bar)
                })
            })
            .when(!show_breadcrumbs, |bar| {
                bar.child(
                    div()
                        .id("omnibar-path-input")
                        .w_full()
                        .min_w_0()
                        .flex_1()
                        .when_some(path_input.as_ref(), |row, input| {
                            row.child(
                                Input::new(input)
                                    .w_full()
                                    .with_size(Size::Medium)
                                    .appearance(false),
                            )
                        }),
                )
            })
    }

    pub fn active_navigation_target(&self, cx: &App) -> NavigationTarget {
        self.active_pane(cx).read(cx).current_navigation_target(cx)
    }

    pub fn toggle_sidebar_collapsed(&mut self, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        config.sidebar_collapsed = !config.sidebar_collapsed;
        let _ = save_config(&config);
        cx.notify();
    }

    pub fn refresh_home_widgets(&mut self, cx: &mut Context<Self>) {
        self.active_pane(cx)
            .update(cx, |pane, cx| pane.reload_home(cx));
        cx.notify();
    }

    pub fn pin_folder_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path_string = path.to_string_lossy().to_string();
        let mut config = load_config().unwrap_or_default();
        if !config.pinned_folders.iter().any(|p| p == &path_string) {
            config.pinned_folders.push(path_string);
            let _ = save_config(&config);
            if let Err(error) = cyberfiles_fs::sync_pin_to_shell_quick_access(&path) {
                eprintln!("[home] pintohome: {error:#}");
            }
            self.refresh_sidebar_cache(cx);
            AppNavigation::refresh_quick_access(cx);
            cx.notify();
        }
    }

    pub fn unpin_folder_path(&mut self, path_string: &str, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        if let Some(index) = config.pinned_folders.iter().position(|p| p == path_string) {
            config.pinned_folders.remove(index);
            let _ = save_config(&config);
            let path = PathBuf::from(path_string);
            if path.exists() {
                if let Err(error) = cyberfiles_fs::sync_unpin_from_shell_quick_access(&path) {
                    eprintln!("[home] unpinfromhome: {error:#}");
                }
            }
            self.refresh_sidebar_cache(cx);
            AppNavigation::refresh_quick_access(cx);
            cx.notify();
        }
    }

    pub fn move_pinned_folder(&mut self, path_string: &str, delta: i32, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        let Some(index) = config.pinned_folders.iter().position(|p| p == path_string) else {
            return;
        };
        let new_index =
            (index as i32 + delta).clamp(0, config.pinned_folders.len() as i32 - 1) as usize;
        if new_index == index {
            return;
        }
        let entry = config.pinned_folders.remove(index);
        config.pinned_folders.insert(new_index, entry);
        let _ = save_config(&config);
        self.refresh_sidebar_cache(cx);
        cx.notify();
    }

    fn pin_current_folder(&mut self, cx: &mut Context<Self>) {
        let pane = self.active_pane(cx);
        let path = pane
            .read(cx)
            .file_browser()
            .read(cx)
            .current_directory()
            .clone();
        let path_string = path.to_string_lossy().to_string();
        let mut config = load_config().unwrap_or_default();
        if let Some(index) = config.pinned_folders.iter().position(|p| p == &path_string) {
            config.pinned_folders.remove(index);
        } else {
            config.pinned_folders.push(path_string);
        }
        let _ = save_config(&config);
        cx.notify();
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

    fn render_content_column(
        &mut self,
        window: &mut Window,
        active_shell: Entity<ShellPanes>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let _ = window;
        let mut col = v_flex()
            .id("content-column")
            .size_full()
            .min_h_0()
            .min_w_0()
            .relative()
            .child(
                div()
                    .id("main-content")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(active_shell),
            )
            .child(self.render_shelf_pane(cx))
            .child(div().flex_shrink_0().child(self.render_status_bar(cx)));

        if self.show_status_center {
            let page = cx.entity();
            let on_close = move |_window: &mut Window, cx: &mut App| {
                let _ = page.update(cx, |page, cx| {
                    page.show_status_center = false;
                    cx.notify();
                });
            };
            col = col.child(
                div()
                    .id("status-center-overlay")
                    .absolute()
                    .bottom(px(36.))
                    .right(px(8.))
                    .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                    .child(crate::status_center::render_status_center_panel(cx, on_close)),
            );
        }

        col
    }

    /// Files `ShelfPane`: shows in-app clipboard staging above the status bar.
    fn render_shelf_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(clipboard) = AppFileClipboard::peek(cx) else {
            return div().id("shelf-pane").flex_shrink_0();
        };
        if clipboard.is_empty() {
            return div().id("shelf-pane").flex_shrink_0();
        }

        let count = clipboard.paths.len();
        let operation_label = match clipboard.operation {
            cyberfiles_fs::ClipboardOperation::Copy => {
                t!("files.shelf.copying", count = count).to_string()
            }
            cyberfiles_fs::ClipboardOperation::Cut => {
                t!("files.shelf.cutting", count = count).to_string()
            }
        };
        let preview = clipboard
            .paths
            .first()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .map(|name| {
                if count <= 1 {
                    t!("files.shelf.preview_one", name = name).to_string()
                } else {
                    t!("files.shelf.preview_many", name = name, rest = count - 1).to_string()
                }
            })
            .unwrap_or_default();

        h_flex()
            .id("shelf-pane")
            .flex_shrink_0()
            .h(px(36.))
            .w_full()
            .px_3()
            .gap_2()
            .items_center()
            .overflow_hidden()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(
                Label::new(operation_label)
                    .text_xs()
                    .text_color(cx.theme().foreground)
                    .flex_shrink_0(),
            )
            .when(!preview.is_empty(), |row| {
                row.child(
                    Label::new(preview)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .flex_1()
                        .min_w_0(),
                )
            })
            .child(
                Button::new("shelf-paste")
                    .label(t!("files.shelf.paste"))
                    .with_size(Size::Small)
                    .flex_shrink_0()
                    .on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(PasteItems), cx);
                    }),
            )
            .child(
                Button::new("shelf-clear")
                    .label(t!("files.shelf.clear"))
                    .with_size(Size::Small)
                    .flex_shrink_0()
                    .on_click(|_, _, cx| {
                        AppFileClipboard::clear(cx);
                    }),
            )
    }

    fn render_shell_layout_row(
        &mut self,
        window: &mut Window,
        active_shell: Entity<ShellPanes>,
        show_info_pane: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let sidebar_sections = self.sidebar_sections.clone();
        h_resizable("main-layout")
            .child(
                resizable_panel()
                    .size(px(240.))
                    .size_range(px(200.)..px(360.))
                    .flex_none()
                    .child(
                        div()
                            .id("sidebar-panel")
                            .size_full()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .child(render_sidebar(
                                cx.entity(),
                                self.active_navigation_target(cx),
                                &sidebar_sections,
                                window,
                                cx,
                            )),
                    ),
            )
            .child(
                resizable_panel().flex_1().min_w_0().child(
                    div()
                        .id("content-region")
                        .size_full()
                        .min_h_0()
                        .min_w_0()
                        .child(
                            h_resizable("main-with-info-pane")
                                .child(resizable_panel().flex_1().min_w_0().child(
                                    self.render_content_column(window, active_shell, cx),
                                ))
                                .child(
                                    resizable_panel()
                                        .size(px(300.))
                                        .size_range(px(220.)..px(480.))
                                        .flex_none()
                                        .visible(show_info_pane)
                                        .child(self.info_pane.clone()),
                                ),
                        ),
                ),
            )
    }

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> TabBar {
        let active = self.active_tab;
        TabBar::new("main-tab-bar")
            .track_scroll(&self.tab_bar_scroll_handle)
            .with_size(Size::Medium)
            .hide_bottom_border()
            .selected_index(active)
            .last_empty_space(
                h_flex().gap_1().pr_1().child(
                    toolbar_icon_button("main-new-tab")
                        .icon(compact_icon(IconName::Plus))
                        .tooltip(t!("nav.new_tab"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.add_tab(NavigationTarget::Path(home_navigation_path()), cx);
                        })),
                ),
            )
            .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                let title = self.tab_title(index, cx);
                let is_selected = index == active;
                let close_color = if is_selected {
                    cx.theme().tab_active_foreground
                } else {
                    cx.theme().muted_foreground
                };
                Tab::new()
                    .w(TITLE_TAB_WIDTH)
                    .min_w(TITLE_TAB_WIDTH)
                    .max_w(TITLE_TAB_WIDTH)
                    .flex_shrink_0()
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .overflow_hidden()
                            .flex()
                            .items_center()
                            .child(Label::new(title).text_left().truncate()),
                    )
                    .suffix(
                        Button::new(format!("main-tab-close-{}", tab.id))
                            .small()
                            .ghost()
                            .mr(TITLE_TAB_CLOSE_RIGHT_INSET)
                            .text_color(close_color)
                            .icon(compact_icon(IconName::Close).small())
                            .tooltip(t!("nav.close_tab"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.close_tab(index, cx);
                            })),
                    )
            }))
            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                this.active_tab = *ix;
                this.pending_tab_scroll_to_ix = Some(*ix);
                this.persist_session(cx);
                cx.notify();
            }))
    }

    /// Menu + tabs + window actions in one row (browser-style title bar).
    fn render_title_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let notifications_count = window.notifications(cx).len();
        let is_dark = cx.theme().mode.is_dark();
        let theme_icon = if is_dark {
            IconName::Moon
        } else {
            IconName::Sun
        };
        let app_menu_bar = app_menus::menu_bar(cx);
        if let Some(ix) = self.pending_tab_scroll_to_ix.take() {
            self.tab_bar_scroll_handle.scroll_to_item(ix);
        }
        let tab_bar = self.render_tab_bar(cx);

        TitleBar::new().child(
            h_flex()
                .id("title-bar-inner")
                .h_full()
                .w_full()
                .min_w_0()
                .items_center()
                .gap_1()
                .child(div().flex_none().child(app_menu_bar))
                .child(
                    div()
                        .id("title-bar-tabs")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .overflow_hidden()
                        .items_center()
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .h(TITLE_TAB_BAR_HEIGHT)
                                .overflow_hidden()
                                .child(tab_bar.w_full().min_w_0().h(TITLE_TAB_BAR_HEIGHT)),
                        ),
                )
                .child(
                    h_flex()
                        .id("title-bar-actions")
                        .flex_none()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            toolbar_icon_button("theme-toggle")
                                .icon(toolbar_icon(theme_icon))
                                .tooltip(t!("nav.theme_toggle"))
                                .on_click(move |_, _, cx| {
                                    let mode = if cx.theme().mode.is_dark() {
                                        ThemeMode::Light
                                    } else {
                                        ThemeMode::Dark
                                    };
                                    apply_theme_mode(mode, cx);
                                }),
                        )
                        .child(
                            toolbar_icon_button("github")
                                .icon(toolbar_icon(IconName::Github))
                                .tooltip(t!("nav.github"))
                                .on_click(|_, _, cx| {
                                    cx.open_url("https://github.com/RGAA-Software/CyberFiles")
                                }),
                        )
                        .child(
                            div().relative().child(
                                Badge::new().count(notifications_count).max(99).child(
                                    toolbar_icon_button("bell")
                                        .icon(toolbar_icon(IconName::Bell))
                                        .tooltip(t!("nav.notifications")),
                                ),
                            ),
                        ),
                ),
        )
    }

    fn render_navigation_toolbar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let show_info_pane = self.show_info_pane;
        let sidebar_collapsed = load_config().map(|c| c.sidebar_collapsed).unwrap_or(false);
        let pane = self.active_pane(cx);
        let target = pane.read(cx).current_navigation_target(cx);
        let browser = pane.read(cx).file_browser();
        let (can_back, can_forward, can_up) = if matches!(
            target,
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin
        ) {
            let b = browser.read(cx);
            (b.can_go_back(), b.can_go_forward(), b.can_go_up())
        } else {
            (false, false, false)
        };
        let show_file_search = matches!(
            target,
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin | NavigationTarget::FileTag(_)
        );

        h_flex()
            .id("navigation-toolbar")
            .w_full()
            .flex_none()
            .min_w_0()
            .h(NAV_TOOLBAR_HEIGHT)
            .min_h(NAV_TOOLBAR_HEIGHT)
            .gap_1()
            .px_1()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            // Files NavigationToolbar col 0: sidebar + back/forward/up/refresh
            .child(
                h_flex()
                    .id("nav-leading")
                    .flex_none()
                    .gap_1()
                    .items_center()
                    .child(
                        toolbar_icon_button("nav-sidebar-toggle")
                            .icon(toolbar_icon(if sidebar_collapsed {
                                IconName::PanelLeftOpen
                            } else {
                                IconName::PanelLeftClose
                            }))
                            .tooltip(t!("sidebar.toggle"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_sidebar_collapsed(cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-back")
                            .icon(toolbar_icon(IconName::ArrowLeft))
                            .tooltip(t!("nav.back"))
                            .disabled(!can_back)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let browser = this.active_pane(cx).read(cx).file_browser().clone();
                                browser.update(cx, |b, cx| b.go_back(cx));
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-forward")
                            .icon(toolbar_icon(IconName::ArrowRight))
                            .tooltip(t!("nav.forward"))
                            .disabled(!can_forward)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let browser = this.active_pane(cx).read(cx).file_browser().clone();
                                browser.update(cx, |b, cx| b.go_forward(cx));
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-up")
                            .icon(toolbar_icon(IconName::ArrowUp))
                            .tooltip(t!("nav.up"))
                            .disabled(!can_up)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let browser = this.active_pane(cx).read(cx).file_browser().clone();
                                browser.update(cx, |b, cx| b.go_up(cx));
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-refresh")
                            .icon(toolbar_icon(IconName::Redo2))
                            .tooltip(t!("nav.refresh"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let pane = this.active_pane(cx);
                                pane.update(cx, |shell, cx| {
                                    shell.file_browser().update(cx, |b, cx| {
                                        b.reload();
                                        cx.notify();
                                    });
                                });
                                cx.notify();
                            })),
                    ),
            )
            // Files col 1: address bar / breadcrumbs (*)
            .child(
                div()
                    .id("nav-omnibar-region")
                    .flex_1()
                    .min_w_0()
                    .h(OMNIBAR_BAR_HEIGHT)
                    .child(self.render_omnibar(window, cx)),
            )
            // Files col 2: split, info pane, pin, search
            .child({
                let mut trailing = h_flex()
                    .id("nav-trailing")
                    .flex_none()
                    .gap_1()
                    .items_center()
                    .child(
                        toolbar_icon_button("nav-split-pane")
                            .icon(toolbar_icon(IconName::LayoutDashboard).path("icons/splitscreen.svg"))
                            .tooltip(t!("nav.split_pane"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_dual_pane(cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-toggle-info")
                            .icon(toolbar_icon(if show_info_pane {
                                IconName::PanelRightClose
                            } else {
                                IconName::PanelRightOpen
                            }))
                            .tooltip(if show_info_pane {
                                t!("nav.hide_info_pane")
                            } else {
                                t!("nav.show_info_pane")
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_info_pane(cx);
                            })),
                    );
                if show_file_search {
                    let search_input = self.ensure_search_input(window, cx);
                    trailing = trailing
                        .child(
                            toolbar_icon_button("nav-pin-folder")
                                .icon(pin_icon())
                                .tooltip(t!("nav.pin_folder"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.pin_current_folder(cx);
                                })),
                        )
                        .child(
                            div()
                                .id("nav-search-wrap")
                                .w(px(200.))
                                .min_w(px(140.))
                                .flex_none()
                                .h(OMNIBAR_BAR_HEIGHT)
                                .flex()
                                .items_center()
                                .child(Input::new(&search_input).w_full().with_size(Size::Medium)),
                        );
                }
                trailing
            })
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.active_pane(cx);
        let target = pane.read(cx).target().clone();

        let (items, selected, hint) = match target {
            NavigationTarget::Path(_)
            | NavigationTarget::RecycleBin
            | NavigationTarget::FileTag(_) => {
                let b = pane.read(cx).file_browser().read(cx);
                let hint = match target {
                    NavigationTarget::RecycleBin => t!("main.status.recycle_bin").to_string(),
                    NavigationTarget::FileTag(_) => t!("main.status.file_tag").to_string(),
                    _ => t!("files.status.local").to_string(),
                };
                (b.item_count(), b.selected_count(), hint)
            }
            NavigationTarget::Home => (0, 0, t!("main.status.home").to_string()),
            NavigationTarget::Settings => (0, 0, t!("main.status.settings").to_string()),
        };

        let status_text = format!(
            "{} {}, {} {}",
            items,
            t!("files.status.items"),
            selected,
            t!("files.status.selected")
        );

        let jobs = TransferStatusGlobal::all_jobs(cx);
        let has_jobs = !jobs.is_empty();
        let active_count = jobs.iter().filter(|j| j.is_active()).count();

        let mut bar = h_flex()
            .id("status-bar")
            .flex_shrink_0()
            .h(px(32.))
            .px_3()
            .items_center()
            .justify_between()
            .gap_3()
            .border_t_1()
            .border_color(cx.theme().border);

        bar = bar.child(
            Label::new(status_text)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        );

        if has_jobs {

            bar = bar.child(
                Button::new("status-center-toggle")
                    .label(if active_count > 0 {
                        format!("{} {}", active_count, t!("files.status_center.badge"))
                    } else {
                        t!("files.status_center.title").to_string()
                    })
                    .with_size(Size::Small)
                    .when(active_count > 0, |b| {
                        b.icon(crate::icons::compact_icon(gpui_component::IconName::ArrowUp))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.show_status_center = !this.show_status_center;
                        cx.notify();
                    })),
            );
        }

        bar.child(
            Label::new(hint)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
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
