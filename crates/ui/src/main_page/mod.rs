use std::path::PathBuf;
use std::rc::Rc;

use cyberfiles_core::{load_config, record_path_history, save_config};
use cyberfiles_fs::{
    breadcrumb_root_menu_sections, copy_items, home_navigation_path, list_drives, move_items,
    path_breadcrumbs, DirectoryReadOptions, PathBreadcrumb,
};
use cyberfiles_platform_windows::list_shell_quick_access_folders;
use cyberfiles_commands::FocusOmnibar;
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    input::{Input, InputEvent, InputState},
    notification::Notification,
    resizable::{h_resizable, resizable_panel},
    tab::{Tab, TabBar},
    v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, WindowExt as _,
};
use rust_i18n::t;

use crate::info_pane::InfoPane;
use crate::app_state::breadcrumb_navigation_target;
use crate::sidebar::{render_sidebar, sidebar_cache_key, SidebarSection};
use crate::omnibar::{OmnibarBreadcrumbHost, BREADCRUMB_DRAG_HOVER_OPEN_MS};
use crate::shell::navigation::NavigationTarget;
use crate::shell::{PaneShell, ShellPanes};

struct TabEntry {
    id: u64,
    shell: Entity<ShellPanes>,
}

pub struct MainPage {
    focus_handle: FocusHandle,
    tabs: Vec<TabEntry>,
    active_tab: usize,
    next_tab_id: u64,
    show_info_pane: bool,
    info_pane: Entity<InfoPane>,
    /// When true, show an editable path field instead of breadcrumb segments.
    omnibar_show_full_path: bool,
    omnibar_path_input: Option<Entity<InputState>>,
    _omnibar_path_subscription: Option<Subscription>,
    omnibar_breadcrumb_host: Option<Entity<OmnibarBreadcrumbHost>>,
    breadcrumb_drag_generation: u64,
    search_input: Option<Entity<InputState>>,
    _search_subscription: Option<Subscription>,
    sidebar_sections: Vec<SidebarSection>,
    sidebar_cache_key: u64,
    sidebar_cache_generation: u64,
    sidebar_cache_loading: bool,
}

impl MainPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let show_info_pane = load_config()
            .map(|c| c.show_info_pane)
            .unwrap_or(true);
        let shell = cx.new(|cx| ShellPanes::new(cx, NavigationTarget::Home));
        Self {
            focus_handle: cx.focus_handle(),
            tabs: vec![TabEntry { id: 0, shell }],
            active_tab: 0,
            next_tab_id: 1,
            show_info_pane,
            info_pane: cx.new(|_| InfoPane::new()),
            omnibar_show_full_path: false,
            omnibar_path_input: None,
            _omnibar_path_subscription: None,
            omnibar_breadcrumb_host: None,
            breadcrumb_drag_generation: 0,
            search_input: None,
            _search_subscription: None,
            sidebar_sections: Vec::new(),
            sidebar_cache_key: 0,
            sidebar_cache_generation: 0,
            sidebar_cache_loading: false,
        }
    }

    /// Rebuild sidebar section lists when settings or pins change (async when cache exists).
    pub fn refresh_sidebar_cache(&mut self, cx: &mut Context<Self>) {
        self.sidebar_cache_key = 0;
        self.ensure_sidebar_cache(cx);
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
            let key = load_config()
                .map(|c| sidebar_cache_key(&c))
                .unwrap_or(0);
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

    fn ensure_omnibar_breadcrumb_host(&mut self, cx: &mut Context<Self>) {
        if self.omnibar_breadcrumb_host.is_some() {
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
        self.omnibar_breadcrumb_host = Some(cx.new(|_| {
            OmnibarBreadcrumbHost::new(
                true,
                Vec::new(),
                DirectoryReadOptions::default(),
                None,
                root_menu,
                on_navigate,
                on_navigate_new_tab,
                on_home,
                on_drop_paths,
                on_drag_hover,
                on_show_full_path,
            )
        }));
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
        let result = if copy {
            copy_items(&paths, &dest)
        } else {
            move_items(&paths, &dest)
        };
        match result {
            Ok(()) => {
                let pane = self.active_pane(cx);
                pane.update(cx, |shell, cx| {
                    shell.file_browser().update(cx, |browser, cx| {
                        if *browser.current_directory() == dest {
                            browser.reload();
                        }
                        cx.notify();
                    });
                });
                cx.notify();
            }
            Err(error) => {
                window.push_notification(
                    Notification::error(format!("{}: {error}", t!("files.drop.error"))),
                    cx,
                );
            }
        }
    }

    fn ensure_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(input) = self.search_input.clone() {
            return input;
        }

        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("search.placeholder"))
        });
        self._search_subscription = Some(cx.subscribe(&input, move |page, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                page.apply_search_from_input(cx);
            }
        }));
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
        let page = cx.new(|cx| Self::new(cx));
        crate::app_state::AppNavigation::set(page.clone(), cx);
        page
    }

    fn active_shell(&self) -> Entity<ShellPanes> {
        self.tabs[self.active_tab].shell.clone()
    }

    fn active_pane(&self, cx: &App) -> Entity<PaneShell> {
        self.active_shell().read(cx).active_pane()
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
        cx.notify();
    }

    fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| shell.toggle_dual_pane(cx));
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

    fn ensure_omnibar_path_input(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(input) = self.omnibar_path_input.clone() {
            return input;
        }

        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("nav.path.placeholder"))
        });
        self._omnibar_path_subscription = Some(cx.subscribe(&input, move |page, _, event: &InputEvent, cx| {
            match event {
                InputEvent::PressEnter { .. } => page.submit_omnibar_path(cx),
                InputEvent::Blur => page.dismiss_omnibar_path_edit(cx),
                _ => {}
            }
        }));
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
        match pane.read(cx).target() {
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
        let target = pane.read(cx).target().clone();
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
        }
    }

    fn render_omnibar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let show_breadcrumbs = !self.omnibar_show_full_path;
        let path_input = if self.omnibar_show_full_path {
            Some(self.ensure_omnibar_path_input(window, cx))
        } else {
            None
        };
        let breadcrumbs = self.omnibar_breadcrumbs(cx);
        self.ensure_omnibar_breadcrumb_host(cx);
        let working_directory = self.omnibar_working_directory(cx);
        let read_options = *self
            .active_pane(cx)
            .read(cx)
            .file_browser()
            .read(cx)
            .read_options();
        if let Some(host) = self.omnibar_breadcrumb_host.clone() {
            host.update(cx, |host, _| {
                host.set_path_context(breadcrumbs, working_directory, read_options);
            });
        }
        let breadcrumb_host = self.omnibar_breadcrumb_host.clone().expect("breadcrumb host");

        div()
            .id("omnibar-host")
            .flex_1()
            .min_w_0()
            .relative()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .id("omnibar-bar")
                    .w_full()
                    .min_h(px(32.))
                    .items_center()
                    .gap_1()
                    .px_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .when(show_breadcrumbs, |bar| {
                        bar.child(
                            div()
                                .id("omnibar-breadcrumb-host")
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .child(breadcrumb_host.clone()),
                        )
                    })
                    .when(!show_breadcrumbs, |bar| {
                        bar.child(
                            div()
                                .id("omnibar-path-input")
                                .flex_1()
                                .min_w_0()
                                .when_some(path_input.as_ref(), |row, input| {
                                    row.child(Input::new(input).w_full().small())
                                }),
                        )
                    }),
            )
    }

    pub fn active_navigation_target(&self, cx: &App) -> NavigationTarget {
        self.active_pane(cx).read(cx).target().clone()
    }

    pub fn toggle_sidebar_collapsed(&mut self, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        config.sidebar_collapsed = !config.sidebar_collapsed;
        let _ = save_config(&config);
        cx.notify();
    }

    pub fn pin_folder_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path_string = path.to_string_lossy().to_string();
        let mut config = load_config().unwrap_or_default();
        if !config.pinned_folders.iter().any(|p| p == &path_string) {
            config.pinned_folders.push(path_string);
            let _ = save_config(&config);
            self.refresh_sidebar_cache(cx);
            cx.notify();
        }
    }

    pub fn unpin_folder_path(&mut self, path_string: &str, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        if let Some(index) = config
            .pinned_folders
            .iter()
            .position(|p| p == path_string)
        {
            config.pinned_folders.remove(index);
            let _ = save_config(&config);
            self.refresh_sidebar_cache(cx);
            cx.notify();
        }
    }

    pub fn move_pinned_folder(&mut self, path_string: &str, delta: i32, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        let Some(index) = config
            .pinned_folders
            .iter()
            .position(|p| p == path_string)
        else {
            return;
        };
        let new_index = (index as i32 + delta).clamp(0, config.pinned_folders.len() as i32 - 1)
            as usize;
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
        let path = pane.read(cx).file_browser().read(cx).current_directory().clone();
        let path_string = path.to_string_lossy().to_string();
        let mut config = load_config().unwrap_or_default();
        if let Some(index) = config
            .pinned_folders
            .iter()
            .position(|p| p == &path_string)
        {
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

    fn render_main_column(
        &mut self,
        window: &mut Window,
        active_shell: Entity<ShellPanes>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .min_h_0()
            .child(self.render_navigation_toolbar(window, cx))
            .child(
                div()
                    .id("main-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(active_shell),
            )
            .child(self.render_status_bar(cx))
    }

    fn render_navigation_toolbar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let show_info_pane = self.show_info_pane;
        let dual_pane = self.active_shell().read(cx).dual_pane();
        let pane = self.active_pane(cx);
        let target = pane.read(cx).target().clone();
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
        let show_file_ops = matches!(
            target,
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin
        );

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
                        let browser = this.active_pane(cx).read(cx).file_browser().clone();
                        browser.update(cx, |b, cx| b.go_back(cx));
                    })),
            )
            .child(
                Button::new("nav-forward")
                    .small()
                    .ghost()
                    .icon(IconName::ArrowRight)
                    .disabled(!can_forward)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let browser = this.active_pane(cx).read(cx).file_browser().clone();
                        browser.update(cx, |b, cx| b.go_forward(cx));
                    })),
            )
            .child(
                Button::new("nav-up")
                    .small()
                    .ghost()
                    .icon(IconName::ArrowUp)
                    .disabled(!can_up)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let browser = this.active_pane(cx).read(cx).file_browser().clone();
                        browser.update(cx, |b, cx| b.go_up(cx));
                    })),
            )
            .child(
                Button::new("nav-refresh")
                    .small()
                    .ghost()
                    .icon(IconName::Redo2)
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
            )
            .when(show_file_ops, |bar| {
                bar.child(
                    Button::new("nav-new-folder")
                        .small()
                        .outline()
                        .icon(IconName::Folder)
                        .tooltip(t!("files.new_folder"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            let pane = this.active_pane(cx);
                            pane.update(cx, |shell, cx| {
                                shell.file_browser().update(cx, |b, cx| {
                                    b.create_new_folder(window, cx);
                                });
                            });
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("nav-pin-folder")
                        .small()
                        .outline()
                        .icon(IconName::Star)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pin_current_folder(cx);
                        })),
                )
                .child(
                    Button::new("nav-new-file")
                        .small()
                        .outline()
                        .icon(IconName::File)
                        .tooltip(t!("files.new_file"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            let pane = this.active_pane(cx);
                            pane.update(cx, |shell, cx| {
                                shell.file_browser().update(cx, |b, cx| {
                                    b.create_new_file(window, cx);
                                });
                            });
                            cx.notify();
                        })),
                )
            })
            .child(
                Button::new("nav-split-pane")
                    .small()
                    .ghost()
                    .icon(IconName::LayoutDashboard)
                    .tooltip(t!("nav.split_pane"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_dual_pane(cx);
                    })),
            )
            .child(
                Button::new("nav-toggle-info")
                    .small()
                    .ghost()
                    .icon(if show_info_pane {
                        IconName::PanelRightClose
                    } else {
                        IconName::PanelRightOpen
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_info_pane(cx);
                    })),
            )
            .child(self.render_omnibar(window, cx))
            .when(show_file_ops, |bar| {
                let search_input = self.ensure_search_input(window, cx);
                bar.child(
                    div()
                        .w(px(200.))
                        .min_w(px(140.))
                        .child(Input::new(&search_input).w_full().small()),
                )
            })
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.active_pane(cx);
        let target = pane.read(cx).target().clone();

        let (items, selected, hint) = match target {
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin => {
                let b = pane.read(cx).file_browser().read(cx);
                let hint = if matches!(target, NavigationTarget::RecycleBin) {
                    t!("main.status.recycle_bin").to_string()
                } else {
                    t!("files.status.local").to_string()
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

        h_flex()
            .id("status-bar")
            .h_8()
            .px_3()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                Label::new(status_text)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
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
        let sidebar_sections = self.sidebar_sections.clone();
        let active = self.active_tab;
        let active_shell = self.active_shell();
        let show_info_pane = self.show_info_pane;
        let info_item = self.info_selection(cx);
        self.info_pane.update(cx, |pane, _| pane.set_item(info_item));

        v_flex()
            .id("main-page")
            .size_full()
            .min_h_0()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &FocusOmnibar, window, cx| {
                this.focus_search_input(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    this.dismiss_omnibar_path_edit(cx);
                }
            }))
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
                            .flex_none()
                            .child(
                                div()
                                    .id("sidebar-panel")
                                    .size_full()
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
                                .size_full()
                                .min_h_0()
                                .when(show_info_pane, |this| {
                                    this.child(
                                        h_resizable("main-with-info-pane")
                                            .child(
                                                resizable_panel()
                                                    .flex_1()
                                                    .child(self.render_main_column(
                                                        window,
                                                        active_shell.clone(),
                                                        cx,
                                                    )),
                                            )
                                            .child(
                                                resizable_panel()
                                                    .size(px(300.))
                                                    .size_range(px(220.)..px(480.))
                                                    .child(self.info_pane.clone()),
                                            ),
                                    )
                                })
                                .when(!show_info_pane, |this| {
                                    this.child(self.render_main_column(window, active_shell, cx))
                                }),
                        ),
                    ),
                    ),
            )
    }
}

