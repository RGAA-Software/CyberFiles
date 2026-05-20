use cyberfiles_core::APP_NAME;
use gpui::{Context, IntoElement, Render, Window, *};

pub struct AppView;

impl Render for AppView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .items_center()
            .justify_center()
            .child(APP_NAME)
    }
}
