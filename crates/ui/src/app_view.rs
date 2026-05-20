use cyberfiles_core::APP_NAME;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, h_flex,
    input::{Input, InputEvent, InputState},
    resizable::{h_resizable, resizable_panel},
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    v_flex,
};

#[derive(Clone)]
struct NavItem {
    name: SharedString,
    description: SharedString,
}

pub struct AppView {
    groups: Vec<(&'static str, Vec<NavItem>)>,
    active_group_index: Option<usize>,
    active_index: Option<usize>,
    collapsed: bool,
    search_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search..."));
        let _subscriptions = vec![cx.subscribe(&search_input, |this, _, e, cx| match e {
            InputEvent::Change => {
                this.active_group_index = Some(0);
                this.active_index = Some(0);
                cx.notify()
            }
            _ => {}
        })];

        let groups = vec![(
            "Main",
            vec![NavItem {
                name: "Home".into(),
                description: "CyberFiles workspace".into(),
            }],
        )];

        Self {
            search_input,
            groups,
            active_group_index: Some(0),
            active_index: Some(0),
            collapsed: false,
            _subscriptions,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Render for AppView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search_input.read(cx).value().trim().to_lowercase();

        let groups: Vec<_> = self
            .groups
            .iter()
            .filter_map(|(name, items)| {
                let filtered: Vec<_> = items
                    .iter()
                    .filter(|item| item.name.to_lowercase().contains(&query))
                    .cloned()
                    .collect();

                if filtered.is_empty() {
                    None
                } else {
                    Some((name, filtered))
                }
            })
            .collect();

        let active_group = self.active_group_index.and_then(|i| groups.get(i));
        let active_item = self
            .active_index
            .and(active_group)
            .and_then(|(_, items)| items.get(self.active_index.unwrap()));

        let (page_name, description) = active_item
            .map(|item| (item.name.clone(), item.description.clone()))
            .unwrap_or_else(|| ("".into(), "".into()));

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
                                                                .child("Workspace")
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
                            .children(groups.iter().enumerate().map(
                                |(group_ix, (group_name, items))| {
                                    SidebarGroup::new(**group_name).child(
                                        SidebarMenu::new().children(items.iter().enumerate().map(
                                            |(ix, item)| {
                                                SidebarMenuItem::new(item.name.clone())
                                                    .active(
                                                        self.active_group_index
                                                            == Some(group_ix)
                                                            && self.active_index == Some(ix),
                                                    )
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, _, cx| {
                                                            this.active_group_index =
                                                                Some(group_ix);
                                                            this.active_index = Some(ix);
                                                            cx.notify();
                                                        },
                                                    ))
                                            },
                                        )),
                                    )
                                },
                            )),
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
                            .overflow_y_scroll()
                            .p_4()
                            .when_some(active_item, |this, _| {
                                this.child(
                                    div()
                                        .size_full()
                                        .items_center()
                                        .justify_center()
                                        .text_lg()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("Welcome to {APP_NAME}")),
                                )
                            }),
                    )
                    .into_any_element(),
            )
    }
}
