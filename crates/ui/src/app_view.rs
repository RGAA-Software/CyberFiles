use cyberfiles_core::APP_NAME;
use gpui::{prelude::*, *};
use gpui_component::{
    h_flex,
    input::{Input, InputEvent, InputState},
    resizable::{h_resizable, resizable_panel},
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarItem, SidebarMenu, SidebarMenuItem},
    v_flex, ActiveTheme as _, Collapsible, Icon, IconName, StyledExt as _,
};

use crate::file_browser::FileBrowser;
use crate::i18n::{nav_description, nav_name};
use crate::settings_view::build_settings;
use rust_i18n::t;

#[derive(Clone)]
struct NavItem {
    id: &'static str,
    icon: IconName,
}

pub struct AppView {
    main_nav: Vec<NavItem>,
    settings: NavItem,
    active_page: &'static str,
    collapsed: bool,
    search_input: Entity<InputState>,
    search_placeholder_locale: String,
    file_browser: Entity<FileBrowser>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("search.placeholder")));
        let _subscriptions = vec![cx.subscribe(&search_input, |_, _, e, cx| match e {
            InputEvent::Change => cx.notify(),
            _ => {}
        })];

        let main_nav = vec![
            NavItem {
                id: "home",
                icon: IconName::LayoutDashboard,
            },
            NavItem {
                id: "files",
                icon: IconName::Folder,
            },
        ];

        let settings = NavItem {
            id: "settings",
            icon: IconName::Settings2,
        };

        let file_browser = cx.new(|_| FileBrowser::new());

        Self {
            search_input,
            main_nav,
            settings,
            active_page: "home",
            collapsed: false,
            search_placeholder_locale: crate::i18n::locale().to_string(),
            file_browser,
            _subscriptions,
        }
    }

    fn active_item(&self) -> Option<&NavItem> {
        if self.active_page == self.settings.id {
            Some(&self.settings)
        } else {
            self.main_nav
                .iter()
                .find(|item| item.id == self.active_page)
        }
    }

    fn filtered_main_nav(&self, cx: &Context<Self>) -> Vec<&NavItem> {
        let query = self.search_input.read(cx).value().trim().to_lowercase();
        self.main_nav
            .iter()
            .filter(|item| nav_name(item.id).to_lowercase().contains(&query))
            .collect()
    }

    fn sync_search_placeholder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = crate::i18n::locale().to_string();
        if self.search_placeholder_locale == locale {
            return;
        }
        self.search_placeholder_locale = locale;
        let placeholder = t!("search.placeholder");
        self.search_input.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx);
        });
    }

    fn page_content(&self, id: &str, cx: &Context<Self>) -> AnyElement {
        match id {
            "home" => div()
                .size_full()
                .items_center()
                .justify_center()
                .v_flex()
                .gap_2()
                .child(
                    div()
                        .text_xl()
                        .child(t!("page.home.welcome", app = APP_NAME)),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!("page.home.overview")),
                )
                .into_any_element(),
            "files" => self.file_browser.clone().into_any_element(),
            "settings" => div()
                .id("settings-page")
                .size_full()
                .min_h_0()
                .overflow_hidden()
                .child(build_settings(cx))
                .into_any_element(),
            _ => div()
                .size_full()
                .items_center()
                .justify_center()
                .child(t!("page.unknown"))
                .into_any_element(),
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_search_placeholder(window, cx);

        let active_item = self.active_item();
        let (page_name, description) = active_item
            .map(|item| {
                (
                    SharedString::from(nav_name(item.id)),
                    SharedString::from(nav_description(item.id)),
                )
            })
            .unwrap_or_else(|| (SharedString::default(), SharedString::default()));

        let filtered_main = self.filtered_main_nav(cx);
        let settings = self.settings.clone();
        let settings_active = self.active_page == settings.id;

        h_resizable("app-container")
            .child(
                resizable_panel()
                    .size(px(255.))
                    .size_range(px(200.)..px(320.))
                    .child(
                        Sidebar::new("app-sidebar")
                            .w(relative(1.))
                            .border_0()
                            .collapsed(self.collapsed)
                            .header(
                                v_flex()
                                    .w_full()
                                    .gap_4()
                                    .child(
                                        SidebarHeader::new()
                                            .w_full()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .rounded(cx.theme().radius_lg)
                                                    .bg(cx.theme().primary)
                                                    .text_color(cx.theme().primary_foreground)
                                                    .size_8()
                                                    .flex_shrink_0()
                                                    .when(!self.collapsed, |this| {
                                                        this.child(Icon::new(
                                                            IconName::GalleryVerticalEnd,
                                                        ))
                                                    })
                                                    .when(self.collapsed, |this| {
                                                        this.size_4()
                                                            .bg(cx.theme().transparent)
                                                            .text_color(cx.theme().foreground)
                                                            .child(Icon::new(
                                                                IconName::GalleryVerticalEnd,
                                                            ))
                                                    }),
                                            )
                                            .when(!self.collapsed, |this| {
                                                this.child(
                                                    v_flex()
                                                        .gap_0()
                                                        .text_sm()
                                                        .flex_1()
                                                        .line_height(relative(1.25))
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(APP_NAME)
                                                        .child(
                                                            div()
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                )
                                                                .child(t!("sidebar.workspace"))
                                                                .text_xs(),
                                                        ),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .bg(cx.theme().sidebar_accent)
                                            .rounded_full()
                                            .px_1()
                                            .when(cx.theme().radius.is_zero(), |this| {
                                                this.rounded(px(0.))
                                            })
                                            .flex_1()
                                            .mx_1()
                                            .child(
                                                Input::new(&self.search_input)
                                                    .appearance(false)
                                                    .cleanable(true),
                                            ),
                                    ),
                            )
                            .child(
                                SidebarGroup::new(t!("sidebar.main"))
                                    .collapsed(self.collapsed)
                                    .child(
                                        SidebarMenu::new()
                                            .w_full()
                                            .collapsed(self.collapsed)
                                            .children(filtered_main.into_iter().map(|item| {
                                                let page_id = item.id;
                                                SidebarMenuItem::new(nav_name(page_id))
                                                    .icon(item.icon.clone())
                                                    .active(self.active_page == page_id)
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, _, cx| {
                                                            this.active_page = page_id;
                                                            cx.notify();
                                                        },
                                                    ))
                                            })),
                                    ),
                            )
                            // Sidebar footer is h_flex (row); without flex_1 the group shrinks to content width.
                            .footer(
                                v_flex().flex_1().w_full().min_w_0().child(
                                    SidebarMenu::new()
                                        .w_full()
                                        .collapsed(self.collapsed)
                                        .child(
                                            SidebarMenuItem::new(nav_name(settings.id))
                                                .icon(settings.icon)
                                                .active(settings_active)
                                                .on_click(cx.listener(
                                                    move |this, _: &ClickEvent, _, cx| {
                                                        this.active_page = "settings";
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .render("app-sidebar-settings", window, cx),
                                ),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .overflow_x_hidden()
                    .child(
                        h_flex()
                            .id("header")
                            .p_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .justify_between()
                            .items_start()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(div().text_xl().child(page_name))
                                    .child(
                                        div()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(description),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("content")
                            .flex_1()
                            .min_h_0()
                            .when(active_item.map(|i| i.id) == Some("settings"), |this| {
                                this.overflow_hidden()
                            })
                            .when(active_item.map(|i| i.id) != Some("settings"), |this| {
                                this.overflow_y_scroll().p_4()
                            })
                            .when_some(active_item, |this, item| {
                                this.child(self.page_content(item.id, cx))
                            }),
                    )
                    .into_any_element(),
            )
    }
}
