use std::path::PathBuf;

use cyberfiles_core::pinned_folder_paths;
use cyberfiles_fs::{list_drives, list_recent_files, DriveInfo, RecentItem};
use gpui::{prelude::*, *};
use gpui_component::{
    alert::Alert,
    button::{Button, ButtonVariants as _},
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex,
    label::Label,
    v_flex, Icon, IconName, Sizable as _,
};
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
                "home-network",
                t!("home.widget.network"),
                t!("home.widget.network.placeholder"),
                IconName::Globe,
            ))
            .child(widget_section(
                "home-tags",
                t!("home.widget.tags"),
                t!("home.widget.tags.placeholder"),
                IconName::Inbox,
            ))
            .child(widget_recent(cx, &recent))
    }
}

fn group_title(icon: IconName, title: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .gap_2()
        .items_center()
        .child(Icon::new(icon).small())
        .child(Label::new(title).text_sm())
}

fn widget_pinned(cx: &mut Context<HomePage>, pinned: &[PathBuf]) -> impl IntoElement {
    GroupBox::new()
        .id("home-pinned")
        .outline()
        .w_full()
        .title(group_title(IconName::Star, t!("home.widget.quick_access")))
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .when(pinned.is_empty(), |body| {
                    body.child(Alert::info(
                        "home-pinned-empty",
                        t!("home.widget.quick_access.empty").to_string(),
                    ))
                })
                .when(!pinned.is_empty(), |body| {
                    body.children(pinned.iter().enumerate().map(|(index, path)| {
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
                    }))
                }),
        )
}

fn widget_section(
    id: &'static str,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    icon: IconName,
) -> impl IntoElement {
    GroupBox::new()
        .id(id)
        .outline()
        .w_full()
        .title(group_title(icon, title))
        .child(Alert::info(
            format!("{id}-placeholder"),
            description.into().to_string(),
        ))
}

fn widget_recent(cx: &mut Context<HomePage>, recent: &[RecentItem]) -> impl IntoElement {
    GroupBox::new()
        .id("home-recent")
        .outline()
        .w_full()
        .title(group_title(IconName::Calendar, t!("home.widget.recent")))
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .when(recent.is_empty(), |body| {
                    body.child(Alert::info(
                        "home-recent-empty",
                        t!("home.widget.recent.empty").to_string(),
                    ))
                })
                .when(!recent.is_empty(), |body| {
                    body.children(recent.iter().enumerate().map(|(index, item)| {
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
                    }))
                }),
        )
}

fn widget_section_drives(cx: &mut Context<HomePage>, drives: &[DriveInfo]) -> impl IntoElement {
    GroupBox::new()
        .id("home-drives")
        .outline()
        .w_full()
        .title(group_title(IconName::HardDrive, t!("home.widget.drives")))
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
