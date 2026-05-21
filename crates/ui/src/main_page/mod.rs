use std::path::PathBuf;

use cyberfiles_core::{load_config, pinned_folder_paths, record_path_history, save_config};
use cyberfiles_fs::{
    breadcrumb_dropdown_entries, home_navigation_path, list_drives, path_breadcrumbs,
    PathBreadcrumb,
};
use cyberfiles_commands::FocusOmnibar;
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    resizable::{h_resizable, resizable_panel},
    sidebar::{
        Sidebar, SidebarGroup, SidebarHeader, SidebarItem, SidebarMenu, SidebarMenuItem,
    },
    tab::{Tab, TabBar},
    v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
};
use rust_i18n::t;

use crate::info_pane::InfoPane;
use crate::omnibar::{
    mode_button_label, mode_placeholder, refresh_suggestions, resolve_path_submit, OmnibarCommand,
    OmnibarMode, OmnibarSuggestion,
};
use crate::shell::navigation::NavigationTarget;
use crate::shell::{PaneShell, ShellPanes};
use cyberfiles_core::APP_NAME;

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
    path_input: Option<Entity<InputState>>,
    _omnibar_subscription: Option<Subscription>,
    omnibar_mode: OmnibarMode,
    omnibar_editing: bool,
    omnibar_suggestions: Vec<OmnibarSuggestion>,
    search_input: Option<Entity<InputState>>,
    _search_subscription: Option<Subscription>,
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
            path_input: None,
            _omnibar_subscription: None,
            omnibar_mode: OmnibarMode::Path,
            omnibar_editing: false,
            omnibar_suggestions: Vec::new(),
            search_input: None,
            _search_subscription: None,
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
        self.omnibar_editing = false;
        cx.notify();
    }

    fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| shell.toggle_dual_pane(cx));
        cx.notify();
    }

    fn omnibar_text(&self, cx: &App) -> String {
        let pane = self.active_pane(cx);
        let target = pane.read(cx).target().clone();
        if matches!(&target, NavigationTarget::Path(_)) {
            pane.read(cx)
                .file_browser()
                .read(cx)
                .current_directory()
                .to_string_lossy()
                .to_string()
        } else {
            target.toolbar_path_label()
        }
    }

    fn ensure_path_input(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(input) = self.path_input.clone() {
            return input;
        }

        let path = self.omnibar_text(cx);
        let placeholder = mode_placeholder(self.omnibar_mode);
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(path)
                .placeholder(placeholder)
        });
        self._omnibar_subscription = Some(cx.subscribe(&input, move |page, _, event: &InputEvent, cx| {
            match event {
                InputEvent::PressEnter { .. } => page.submit_omnibar(cx),
                InputEvent::Change => page.refresh_omnibar_suggestions(cx),
                InputEvent::Focus => {
                    page.omnibar_editing = true;
                    page.refresh_omnibar_suggestions(cx);
                    cx.notify();
                }
                InputEvent::Blur => {
                    page.omnibar_editing = false;
                    cx.notify();
                }
            }
        }));
        self.path_input = Some(input.clone());
        input
    }

    fn sync_omnibar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.ensure_path_input(window, cx);
        let placeholder = mode_placeholder(self.omnibar_mode);
        input.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx);
        });

        if !self.omnibar_editing {
            let text = self.omnibar_text(cx);
            input.update(cx, |state, cx| {
                state.set_value(text, window, cx);
            });
        }
    }

    pub fn focus_omnibar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.omnibar_editing = true;
        self.omnibar_mode = OmnibarMode::Path;
        let input = self.ensure_path_input(window, cx);
        let text = self.omnibar_text(cx);
        input.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.focus(window, cx);
        });
        self.refresh_omnibar_suggestions(cx);
        cx.notify();
    }

    fn refresh_omnibar_suggestions(&mut self, cx: &mut Context<Self>) {
        let query = self
            .path_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        self.omnibar_suggestions = refresh_suggestions(self.omnibar_mode, &query);
        cx.notify();
    }

    fn toggle_omnibar_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.omnibar_mode = match self.omnibar_mode {
            OmnibarMode::Path => OmnibarMode::CommandPalette,
            OmnibarMode::CommandPalette => OmnibarMode::Path,
        };
        self.omnibar_editing = true;
        let input = self.ensure_path_input(window, cx);
        input.update(cx, |state, cx| {
            state.set_placeholder(mode_placeholder(self.omnibar_mode), window, cx);
            state.set_value(String::new(), window, cx);
            state.focus(window, cx);
        });
        self.refresh_omnibar_suggestions(cx);
        cx.notify();
    }

    fn start_omnibar_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.omnibar_editing = true;
        self.omnibar_mode = OmnibarMode::Path;
        let input = self.ensure_path_input(window, cx);
        let text = self.omnibar_text(cx);
        input.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.focus(window, cx);
        });
        self.refresh_omnibar_suggestions(cx);
        cx.notify();
    }

    fn apply_omnibar_suggestion(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(suggestion) = self.omnibar_suggestions.get(index).cloned() else {
            return;
        };
        match suggestion {
            OmnibarSuggestion::Path { path, label } => {
                if self.omnibar_mode == OmnibarMode::Path {
                    if let Some(input) = self.path_input.clone() {
                        input.update(cx, |state, cx| {
                            state.set_value(label, window, cx);
                        });
                    }
                    self.omnibar_editing = false;
                    self.navigate_to(NavigationTarget::Path(path), cx);
                }
            }
            OmnibarSuggestion::Command { id, .. } => {
                self.execute_omnibar_command(id, cx);
                self.omnibar_editing = false;
            }
        }
        self.omnibar_suggestions.clear();
        cx.notify();
    }

    fn execute_omnibar_command(&mut self, command: OmnibarCommand, cx: &mut Context<Self>) {
        match command {
            OmnibarCommand::NavigateHome => self.navigate_to(NavigationTarget::Home, cx),
            OmnibarCommand::OpenSettings => self.navigate_to(NavigationTarget::Settings, cx),
            OmnibarCommand::OpenRecycleBin => self.navigate_to(NavigationTarget::RecycleBin, cx),
            OmnibarCommand::ToggleDualPane => self.toggle_dual_pane(cx),
            OmnibarCommand::ToggleInfoPane => self.toggle_info_pane(cx),
            OmnibarCommand::NewTab => {
                self.add_tab(NavigationTarget::Path(home_navigation_path()), cx);
            }
        }
    }

    fn submit_omnibar(&mut self, cx: &mut Context<Self>) {
        if self.omnibar_mode == OmnibarMode::CommandPalette {
            if let Some(OmnibarSuggestion::Command { id, .. }) = self.omnibar_suggestions.first() {
                let id = *id;
                self.execute_omnibar_command(id, cx);
                self.omnibar_suggestions.clear();
                self.omnibar_editing = false;
                return;
            }
        }

        let Some(input) = self.path_input.clone() else {
            return;
        };
        let text = input.read(cx).value().to_string();
        if let Some(target) = resolve_path_submit(&text) {
            if let NavigationTarget::Path(ref path) = target {
                record_path_history(path);
            }
            self.omnibar_editing = false;
            self.omnibar_suggestions.clear();
            self.navigate_to(target, cx);
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
            NavigationTarget::Home => vec![PathBreadcrumb {
                label: t!("nav.home").to_string(),
                path: PathBuf::from("home"),
            }],
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

    fn render_omnibar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.sync_omnibar(window, cx);
        let path_input = self.ensure_path_input(window, cx);
        let mode = self.omnibar_mode;
        let editing = self.omnibar_editing;
        let suggestions = self.omnibar_suggestions.clone();
        let breadcrumbs = self.omnibar_breadcrumbs(cx);
        let show_breadcrumbs = mode == OmnibarMode::Path && !editing;

        div()
            .id("omnibar-host")
            .flex_1()
            .min_w_0()
            .relative()
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
                    .child(
                        Button::new("omnibar-mode")
                            .xsmall()
                            .ghost()
                            .label(mode_button_label(mode))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_omnibar_mode(window, cx);
                            })),
                    )
                    .when(show_breadcrumbs, |bar| {
                        bar.child(
                            h_flex()
                                .id("omnibar-breadcrumbs")
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .items_center()
                                .overflow_x_scroll()
                                .children(breadcrumbs.iter().enumerate().map(
                                    |(index, crumb)| {
                                        let is_last = index + 1 == breadcrumbs.len();
                                        let path_nav = crumb.path.clone();
                                        let path_menu = crumb.path.clone();
                                        let label = crumb.label.clone();
                                        let show_sep = !is_last;
                                        h_flex()
                                            .items_center()
                                            .child(
                                                Button::new(("omnibar-crumb", index))
                                                    .xsmall()
                                                    .ghost()
                                                    .label(label)
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, _, cx| {
                                                            if is_last {
                                                                return;
                                                            }
                                                            let target = crate::app_state::breadcrumb_navigation_target(
                                                                &path_nav,
                                                            );
                                                            this.navigate_to(target, cx);
                                                        },
                                                    )),
                                            )
                                            .child(
                                                Button::new(("omnibar-crumb-menu", index))
                                                    .xsmall()
                                                    .ghost()
                                                    .icon(IconName::ChevronDown)
                                                    .dropdown_menu_with_anchor(
                                                        Anchor::BottomLeft,
                                                        move |menu, _, _| {
                                                            let entries =
                                                                breadcrumb_dropdown_entries(
                                                                    &path_menu,
                                                                );
                                                            let mut menu = menu.scrollable(true);
                                                            if entries.is_empty() {
                                                                menu = menu.item(
                                                                    PopupMenuItem::new(t!(
                                                                        "omnibar.breadcrumb.empty"
                                                                    ))
                                                                    .disabled(true),
                                                                );
                                                            } else {
                                                                for entry in entries {
                                                                    let target =
                                                                        entry.path.clone();
                                                                    let entry_label =
                                                                        entry.label.clone();
                                                                    menu = menu.item(
                                                                        PopupMenuItem::new(
                                                                            entry_label,
                                                                        )
                                                                        .on_click(
                                                                            move |_, _, cx| {
                                                                                crate::app_state::AppNavigation::navigate_to_path(
                                                                                    target.clone(),
                                                                                    cx,
                                                                                );
                                                                            },
                                                                        ),
                                                                    );
                                                                }
                                                            }
                                                            menu
                                                        },
                                                    ),
                                            )
                                            .when(show_sep, |row| {
                                                row.child(
                                                    Icon::new(IconName::ChevronRight).small(),
                                                )
                                            })
                                    },
                                )),
                        )
                    })
                    .when(!show_breadcrumbs, |bar| {
                        bar.child(div().flex_1().min_w_0().child(Input::new(&path_input).w_full().small()))
                    }),
            )
            .when(editing && !suggestions.is_empty(), |host| {
                host.child(
                    div()
                        .id("omnibar-suggestions")
                        .absolute()
                        .top_8()
                        .left_0()
                        .w_full()
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .shadow_md()
                        .py_1()
                        .children(suggestions.into_iter().enumerate().map(|(index, item)| {
                            let label = match &item {
                                OmnibarSuggestion::Path { label, .. } => label.clone(),
                                OmnibarSuggestion::Command { label, .. } => label.clone(),
                            };
                            Button::new(("omnibar-suggestion", index))
                                .ghost()
                                .small()
                                .w_full()
                                .label(label)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.apply_omnibar_suggestion(index, window, cx);
                                }))
                        })),
                )
            })
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
                        let pane = this.active_pane(cx);
                        pane.update(cx, |shell, cx| {
                            if let NavigationTarget::Path(_) = shell.target() {
                                shell.file_browser().update(cx, |b, cx| {
                                    b.go_back(cx);
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
                        let pane = this.active_pane(cx);
                        pane.update(cx, |shell, cx| {
                            shell.file_browser().update(cx, |b, cx| {
                                b.go_forward(cx);
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
                        let pane = this.active_pane(cx);
                        pane.update(cx, |shell, cx| {
                            shell.file_browser().update(cx, |b, cx| {
                                b.go_up(cx);
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
                        .label(t!("files.new_folder"))
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
                        .label(t!("files.new_file"))
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

    fn render_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drives = list_drives();
        let pinned = pinned_folder_paths();

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
                                    this.navigate_to(NavigationTarget::Home, cx);
                                })),
                        ),
                    ),
            )
            .child(
                SidebarGroup::new(t!("sidebar.section.pinned")).child(
                    SidebarMenu::new().w_full().children(pinned.into_iter().map(|path| {
                        let label = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.to_string_lossy().to_string());
                        SidebarMenuItem::new(label)
                            .icon(IconName::Star)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.navigate_to(NavigationTarget::Path(path.clone()), cx);
                            }))
                    })),
                ),
            )
            .child(
                SidebarGroup::new(t!("sidebar.section.places")).child(
                    SidebarMenu::new().w_full().child(
                        SidebarMenuItem::new(t!("nav.recycle_bin"))
                            .icon(IconName::Delete)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_to(NavigationTarget::RecycleBin, cx);
                            })),
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
                                    this.navigate_to(NavigationTarget::Path(path.clone()), cx);
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
                                    this.navigate_to(NavigationTarget::Settings, cx);
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
                this.focus_omnibar(window, cx);
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
                            .child(self.render_sidebar(window, cx)),
                    )
                    .child(
                        resizable_panel().flex_1().child(
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
