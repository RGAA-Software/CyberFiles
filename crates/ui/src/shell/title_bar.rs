use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, SharedString, Styled as _, Window, div,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, ThemeMode, Sizable as _, TitleBar, WindowExt as _,
    badge::Badge,
    button::{Button, ButtonVariants as _},
};

use super::app_menus;
use super::preferences::apply_theme_mode;

pub struct AppTitleBar {
    app_menu_bar: Entity<gpui_component::menu::AppMenuBar>,
    child: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
}

impl AppTitleBar {
    pub fn new(
        title: impl Into<SharedString>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let app_menu_bar = app_menus::init(title, cx);

        Self {
            app_menu_bar,
            child: Rc::new(|_, _| div().into_any_element()),
        }
    }
}

impl Render for AppTitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notifications_count = window.notifications(cx).len();
        let is_dark = cx.theme().mode.is_dark();
        let theme_icon = if is_dark {
            IconName::Moon
        } else {
            IconName::Sun
        };

        TitleBar::new()
            .child(div().flex().items_center().child(self.app_menu_bar.clone()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .px_2()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child((self.child.clone())(window, cx))
                    .child(
                        Button::new("theme-toggle")
                            .small()
                            .ghost()
                            .icon(Icon::new(theme_icon))
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
                        Button::new("github")
                            .icon(IconName::Github)
                            .small()
                            .ghost()
                            .on_click(|_, _, cx| {
                                cx.open_url("https://github.com/longbridge/gpui-component")
                            }),
                    )
                    .child(
                        div().relative().child(
                            Badge::new().count(notifications_count).max(99).child(
                                Button::new("bell")
                                    .small()
                                    .ghost()
                                    .compact()
                                    .icon(IconName::Bell),
                            ),
                        ),
                    ),
            )
    }
}
