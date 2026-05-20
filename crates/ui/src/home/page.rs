use std::path::PathBuf;

use cyberfiles_core::pinned_folder_paths;
use cyberfiles_fs::{list_drives, list_recent_files, DriveInfo, RecentItem};
use gpui::{prelude::*, *};
use gpui_component::{button::{Button, ButtonVariants as _}, h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _};
use rust_i18n::t;

use crate::app_state::AppNavigation;

pub struct HomePage;

impl HomePage {
    pub fn view(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Render for HomePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drives = list_drives();
        let pinned = pinned_folder_paths();
        let recent = list_recent_files();

        v_flex()
            .id("home-page")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .p_4()
            .gap_4()
            .child(widget_pinned(cx, &pinned))
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
            .child(widget_recent(cx, &recent))
    }
}

fn widget_pinned(cx: &mut Context<HomePage>, pinned: &[PathBuf]) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(IconName::Star).small())
                .child(div().text_sm().child(t!("home.widget.quick_access"))),
        )
        .when(pinned.is_empty(), |section| {
            section.child(
                div()
                    .w_full()
                    .p_4()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("home.widget.quick_access.empty")),
            )
        })
        .when(!pinned.is_empty(), |section| {
            section.child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .children(pinned.iter().enumerate().map(|(index, path)| {
                        let label = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.to_string_lossy().to_string());
                        let path = path.clone();
                        h_flex()
                            .id(("home-pinned", index))
                            .w_full()
                            .child(
                                Button::new(("home-pinned-btn", index))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Folder)
                                    .label(label)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        AppNavigation::navigate_to_path(path.clone(), cx);
                                    })),
                        )
                    })),
            )
        })
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

fn widget_recent(cx: &mut Context<HomePage>, recent: &[RecentItem]) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(IconName::Calendar).small())
                .child(div().text_sm().child(t!("home.widget.recent"))),
        )
        .when(recent.is_empty(), |section| {
            section.child(
                div()
                    .w_full()
                    .p_4()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("home.widget.recent.empty")),
            )
        })
        .when(!recent.is_empty(), |section| {
            section.child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .children(recent.iter().enumerate().map(|(index, item)| {
                        let path = item.path.clone();
                        let label = item.label.clone();
                        h_flex()
                            .id(("home-recent", index))
                            .w_full()
                            .child(
                                Button::new(("home-recent-btn", index))
                                    .ghost()
                                    .small()
                                    .icon(IconName::File)
                                    .label(label)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        AppNavigation::navigate_to_path(path.clone(), cx);
                                    })),
                            )
                    })),
            )
        })
}

fn widget_section_drives(cx: &mut Context<HomePage>, drives: &[DriveInfo]) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(IconName::HardDrive).small())
                .child(div().text_sm().child(t!("home.widget.drives"))),
        )
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .children(drives.iter().enumerate().map(|(index, drive)| {
                    let path = drive.path.clone();
                    h_flex()
                        .id(("home-drive", index))
                        .w_full()
                        .child(
                            Button::new(("home-drive-btn", index))
                                .ghost()
                                .small()
                                .icon(IconName::HardDrive)
                                .label(drive.label.clone())
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    AppNavigation::navigate_to_path(path.clone(), cx);
                                })),
                        )
                })),
        )
}
