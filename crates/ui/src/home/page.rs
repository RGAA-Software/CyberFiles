use cyberfiles_fs::{list_drives, DriveInfo};
use gpui::{prelude::*, *};
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _};
use rust_i18n::t;

pub struct HomePage;

impl HomePage {
    pub fn render(cx: &App) -> impl IntoElement {
        let drives = list_drives();

        v_flex()
            .id("home-page")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .p_4()
            .gap_4()
            .child(widget_section(
                cx,
                t!("home.widget.quick_access"),
                t!("home.widget.quick_access.placeholder"),
                IconName::Star,
            ))
            .child(widget_section_drives(cx, &drives))
            .child(widget_section(
                cx,
                t!("home.widget.network"),
                t!("home.widget.network.placeholder"),
                IconName::Globe,
            ))
            .child(widget_section(
                cx,
                t!("home.widget.tags"),
                t!("home.widget.tags.placeholder"),
                IconName::Inbox,
            ))
            .child(widget_section(
                cx,
                t!("home.widget.recent"),
                t!("home.widget.recent.placeholder"),
                IconName::Calendar,
            ))
    }
}

fn widget_section(
    cx: &App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    icon: IconName,
) -> impl IntoElement {
    let title: SharedString = title.into();
    let description: SharedString = description.into();

    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(icon).small())
                .child(div().text_sm().child(title)),
        )
        .child(
            div()
                .w_full()
                .p_4()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().muted)
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(description),
        )
}

fn widget_section_drives(cx: &App, drives: &[DriveInfo]) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(IconName::HardDrive).small())
                .child(
                    div().text_sm().child(t!("home.widget.drives")),
                ),
        )
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .children(drives.iter().enumerate().map(|(index, drive)| {
                    h_flex()
                        .id(("home-drive", index))
                        .w_full()
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::HardDrive).small())
                        .child(drive.label.clone())
                })),
        )
}
