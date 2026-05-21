use gpui::{
    prelude::FluentBuilder, AnyView, App, AppContext, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, SharedString, Styled,
    Window, div,
};
use gpui_component::{Root, v_flex};

use crate::app_state::AppNavigation;
use super::title_bar::AppTitleBar;

/// Window chrome: custom title bar + main content + overlay layers.
pub struct AppShell {
    focus_handle: FocusHandle,
    title_bar: Entity<AppTitleBar>,
    view: AnyView,
}

impl AppShell {
    pub fn new(
        title: impl Into<SharedString>,
        view: impl Into<AnyView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new(title, window, cx));
        Self {
            focus_handle: cx.focus_handle(),
            title_bar,
            view: view.into(),
        }
    }
}

impl Focusable for AppShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let path_edit_active = cx
            .try_global::<AppNavigation>()
            .is_some_and(|nav| nav.main_page().read(cx).omnibar_path_edit_active());

        div()
            .id("app-shell")
            .size_full()
            .when(path_edit_active, |shell| {
                shell.on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| {
                    if let Some(nav) = cx.try_global::<AppNavigation>() {
                        nav.main_page().update(cx, |page, cx| {
                            page.dismiss_omnibar_path_edit(cx);
                        });
                    }
                }))
            })
            .child(
                v_flex()
                    .size_full()
                    .child(self.title_bar.clone())
                    .child(
                        div()
                            .track_focus(&self.focus_handle)
                            .flex_1()
                            .overflow_hidden()
                            .child(self.view.clone()),
                    )
                    .children(sheet_layer)
                    .children(dialog_layer)
                    .children(notification_layer),
            )
    }
}
