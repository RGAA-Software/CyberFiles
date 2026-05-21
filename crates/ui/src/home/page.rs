use std::collections::HashSet;
use std::path::PathBuf;

use cyberfiles_core::{load_config, pinned_folder_paths, FileTagConfig};
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
#[cfg(windows)]
use cyberfiles_platform_windows::{
    list_known_folder_folders, list_shell_quick_access_folders, FOLDERID_NETWORK,
};

pub struct HomePage;

impl HomePage {
    pub fn view(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Render for HomePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drives = list_drives();
        let quick_access = quick_access_entries();
        let network = network_entries();
        let tags = load_config()
            .map(|c| c.file_tags)
            .unwrap_or_default();
        let recent = list_recent_files();

        v_flex()
            .id("home-page")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .p_4()
            .gap_4()
            .child(widget_quick_access(cx, &quick_access))
            .child(widget_section_drives(cx, &drives))
            .child(widget_network(cx, &network))
            .child(widget_file_tags(cx, &tags))
            .child(widget_recent(cx, &recent))
    }
}

struct QuickAccessEntry {
    label: String,
    path: PathBuf,
}

fn quick_access_entries() -> Vec<QuickAccessEntry> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    #[cfg(windows)]
    if let Ok(shell) = list_shell_quick_access_folders() {
        for item in shell {
            if item.path.exists() && seen.insert(path_key(&item.path)) {
                entries.push(QuickAccessEntry {
                    label: item.display_name,
                    path: item.path,
                });
            }
        }
    }

    for path in pinned_folder_paths() {
        if !path.exists() || !seen.insert(path_key(&path)) {
            continue;
        }
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        entries.push(QuickAccessEntry { label, path });
    }

    entries
}

#[cfg(windows)]
fn network_entries() -> Vec<QuickAccessEntry> {
    list_known_folder_folders(&FOLDERID_NETWORK)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| !e.path.as_os_str().is_empty())
        .map(|e| QuickAccessEntry {
            label: e.display_name,
            path: e.path,
        })
        .collect()
}

#[cfg(not(windows))]
fn network_entries() -> Vec<QuickAccessEntry> {
    Vec::new()
}

fn path_key(path: &std::path::Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

fn group_title(icon: IconName, title: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .gap_2()
        .items_center()
        .child(Icon::new(icon).small())
        .child(Label::new(title).text_sm())
}

fn widget_quick_access(cx: &mut Context<HomePage>, entries: &[QuickAccessEntry]) -> impl IntoElement {
    GroupBox::new()
        .id("home-quick-access")
        .outline()
        .w_full()
        .title(group_title(IconName::Star, t!("home.widget.quick_access")))
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .when(entries.is_empty(), |body| {
                    body.child(Alert::info(
                        "home-quick-access-empty",
                        t!("home.widget.quick_access.empty").to_string(),
                    ))
                })
                .when(!entries.is_empty(), |body| {
                    body.children(entries.iter().enumerate().map(|(index, entry)| {
                        let path = entry.path.clone();
                        let label = entry.label.clone();
                        h_flex()
                            .id(("home-quick-access", index))
                            .w_full()
                            .child(
                                Button::new(("home-quick-access-btn", index))
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

fn widget_network(cx: &mut Context<HomePage>, entries: &[QuickAccessEntry]) -> impl IntoElement {
    GroupBox::new()
        .id("home-network")
        .outline()
        .w_full()
        .title(group_title(IconName::Globe, t!("home.widget.network")))
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .when(entries.is_empty(), |body| {
                    body.child(Alert::info(
                        "home-network-empty",
                        t!("home.widget.network.empty").to_string(),
                    ))
                })
                .when(!entries.is_empty(), |body| {
                    body.children(entries.iter().enumerate().map(|(index, entry)| {
                        let path = entry.path.clone();
                        let label = entry.label.clone();
                        h_flex()
                            .id(("home-network", index))
                            .w_full()
                            .child(
                                Button::new(("home-network-btn", index))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Globe)
                                    .label(label)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        AppNavigation::navigate_to_path(path.clone(), cx);
                                    })),
                            )
                    }))
                }),
        )
}

fn widget_file_tags(cx: &mut Context<HomePage>, tags: &[FileTagConfig]) -> impl IntoElement {
    GroupBox::new()
        .id("home-tags")
        .outline()
        .w_full()
        .title(group_title(IconName::Inbox, t!("home.widget.tags")))
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .when(tags.is_empty(), |body| {
                    body.child(Alert::info(
                        "home-tags-empty",
                        t!("home.widget.tags.empty").to_string(),
                    ))
                })
                .when(!tags.is_empty(), |body| {
                    body.children(tags.iter().enumerate().map(|(index, tag)| {
                        let name = tag.name.clone();
                        h_flex()
                            .id(("home-tag", index))
                            .w_full()
                            .child(
                                Button::new(("home-tag-btn", index))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Inbox)
                                    .label(tag.name.clone())
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        AppNavigation::navigate_to_file_tag(name.clone(), cx);
                                    })),
                            )
                    }))
                }),
        )
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
