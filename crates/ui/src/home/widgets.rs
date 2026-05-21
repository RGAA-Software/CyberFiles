//! Home page widget bodies (Files `*Widget` parity).

use std::path::PathBuf;

use cyberfiles_fs::{
    DriveInfo, FileTagPreview, QuickAccessEntry, RecentItem,
};
use cyberfiles_platform_windows::open_item_properties;
use gpui::{prelude::*, *};
use gpui_component::{
    alert::Alert,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem},
    v_flex, ActiveTheme as _, Icon, IconName, Sizable as _,
};
use rust_i18n::t;

use crate::app_state::AppNavigation;
use crate::home::page::HomePage;
use crate::home::widget_shell::{
    card_grid, shell_icon_for_path, space_progress_bar, CARD_MIN_HEIGHT, CARD_WIDTH,
    FOLDER_CARD_HEIGHT, FOLDER_CARD_WIDTH,
};

#[cfg(windows)]
use cyberfiles_platform_windows::{list_known_folder_folders, FOLDERID_NETWORK};

#[derive(Clone)]
pub struct NetworkEntry {
    pub label: String,
    pub path: PathBuf,
}

pub fn load_network_entries() -> Vec<NetworkEntry> {
    #[cfg(windows)]
    {
        list_known_folder_folders(&FOLDERID_NETWORK)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| !e.path.as_os_str().is_empty())
            .map(|e| NetworkEntry {
                label: e.display_name,
                path: e.path,
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

impl HomePage {
    fn section_header(
        &self,
        id: impl Into<ElementId>,
        icon: IconName,
        title: impl Into<SharedString>,
        expanded: bool,
        section_key: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let chevron = if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        Button::new(id)
            .ghost()
            .small()
            .w_full()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(chevron).small())
                    .child(Icon::new(icon).small())
                    .child(
                        Label::new(title)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_expanded(section_key, cx);
            }))
    }

    pub(super) fn render_quick_access_widget(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        entries: &[QuickAccessEntry],
    ) -> impl IntoElement {
        let expanded = self.prefs.quick_access_expanded;
        v_flex()
            .id("home-widget-quick-access")
            .w_full()
            .gap_1()
            .child(self.section_header(
                "home-qa-header",
                IconName::Star,
                t!("home.widget.quick_access"),
                expanded,
                "quick_access",
                cx,
            ))
            .when(expanded, |body| {
                body.when(entries.is_empty(), |b| {
                    b.child(Alert::info(
                        "home-quick-access-empty",
                        t!("home.widget.quick_access.empty").to_string(),
                    ))
                })
                .when(!entries.is_empty(), |b| {
                    b.child(card_grid(entries.iter().enumerate().map(|(index, entry)| {
                        self.folder_card(index, "home-qa", entry, cx)
                            .into_any_element()
                    })))
                })
            })
    }

    pub(super) fn render_drives_widget(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        drives: &[DriveInfo],
    ) -> impl IntoElement {
        let expanded = self.prefs.drives_expanded;
        v_flex()
            .id("home-widget-drives")
            .w_full()
            .gap_1()
            .child(self.section_header(
                "home-drives-header",
                IconName::HardDrive,
                t!("home.widget.drives"),
                expanded,
                "drives",
                cx,
            ))
            .when(expanded, |body| {
                body.child(card_grid(drives.iter().enumerate().map(|(index, drive)| {
                    self.drive_card(index, "home-drive", drive, cx)
                        .into_any_element()
                })))
            })
    }

    pub(super) fn render_network_widget(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        entries: &[NetworkEntry],
    ) -> impl IntoElement {
        let expanded = self.prefs.network_expanded;
        v_flex()
            .id("home-widget-network")
            .w_full()
            .gap_1()
            .child(self.section_header(
                "home-network-header",
                IconName::Globe,
                t!("home.widget.network"),
                expanded,
                "network",
                cx,
            ))
            .when(expanded, |body| {
                body.when(entries.is_empty(), |b| {
                    b.child(Alert::info(
                        "home-network-empty",
                        t!("home.widget.network.empty").to_string(),
                    ))
                })
                .when(!entries.is_empty(), |b| {
                    b.child(card_grid(entries.iter().enumerate().map(|(index, entry)| {
                        let drive = DriveInfo {
                            path: entry.path.clone(),
                            label: entry.label.clone(),
                            volume_label: None,
                            total_bytes: None,
                            free_bytes: None,
                            is_removable: false,
                            is_network: true,
                        };
                        self.drive_card(index, "home-network", &drive, cx)
                            .into_any_element()
                    })))
                })
            })
    }

    pub(super) fn render_file_tags_widget(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        previews: &[FileTagPreview],
    ) -> impl IntoElement {
        let expanded = self.prefs.file_tags_expanded;
        v_flex()
            .id("home-widget-tags")
            .w_full()
            .gap_1()
            .child(self.section_header(
                "home-tags-header",
                IconName::Inbox,
                t!("home.widget.tags"),
                expanded,
                "file_tags",
                cx,
            ))
            .when(expanded, |body| {
                body.when(previews.is_empty(), |b| {
                    b.child(Alert::info(
                        "home-tags-empty",
                        t!("home.widget.tags.empty").to_string(),
                    ))
                })
                .when(!previews.is_empty(), |b| {
                    b.child(card_grid(previews.iter().enumerate().map(|(index, preview)| {
                        self.tag_container(index, preview, cx).into_any_element()
                    })))
                })
            })
    }

    pub(super) fn render_recent_widget(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        recent: &[RecentItem],
    ) -> impl IntoElement {
        let expanded = self.prefs.recent_expanded;
        v_flex()
            .id("home-widget-recent")
            .w_full()
            .gap_1()
            .child(self.section_header(
                "home-recent-header",
                IconName::Calendar,
                t!("home.widget.recent"),
                expanded,
                "recent",
                cx,
            ))
            .when(expanded, |body| {
                body.when(recent.is_empty(), |b| {
                    b.child(Alert::info(
                        "home-recent-empty",
                        t!("home.widget.recent.empty").to_string(),
                    ))
                })
                .when(!recent.is_empty(), |b| {
                    b.child(v_flex().w_full().gap_px().children(
                        recent.iter().enumerate().map(|(index, item)| {
                            self.recent_row(index, item, cx).into_any_element()
                        }),
                    ))
                })
            })
    }

    fn folder_card(
        &self,
        index: usize,
        prefix: &str,
        entry: &QuickAccessEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let path = entry.path.clone();
        let label = entry.label.clone();
        let pinned = entry.is_pinned;
        Button::new(SharedString::from(format!("{prefix}-folder-{index}")))
            .ghost()
            .w(FOLDER_CARD_WIDTH)
            .min_h(FOLDER_CARD_HEIGHT)
            .child(
                v_flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .child(
                        div()
                            .relative()
                            .child(shell_icon_for_path(&entry.path).size_8())
                            .when(pinned, |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .right_0()
                                        .child(Icon::new(IconName::Star).xsmall()),
                                )
                            }),
                    )
                    .child(Label::new(label).text_sm().text_center()),
            )
            .on_click(cx.listener({
                let path = path.clone();
                move |_, event, window, cx| {
                    open_path(&path, event, window, cx);
                }
            }))
            .context_menu({
                let path = path.clone();
                let pinned = pinned;
                move |menu, window, cx| {
                    folder_context_menu(menu, &path, pinned, window, cx)
                }
            })
    }

    fn drive_card(
        &self,
        index: usize,
        prefix: &str,
        drive: &DriveInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let path = drive.path.clone();
        let title = drive.label.clone();
        let space = drive.space_text();
        let frac = drive.used_fraction();
        Button::new(SharedString::from(format!("{prefix}-drive-{index}")))
            .ghost()
            .w(CARD_WIDTH)
            .min_h(CARD_MIN_HEIGHT)
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(shell_icon_for_path(&drive.path).size_8())
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                Label::new(title)
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM),
                            )
                            .when_some(frac, |col, f| {
                                col.child(space_progress_bar(
                                    SharedString::from(format!("{prefix}-bar-{index}")),
                                    f,
                                ))
                            })
                            .when_some(space, |col, text| {
                                col.child(
                                    Label::new(text)
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                )
                            }),
                    ),
            )
            .on_click(cx.listener({
                let path = path.clone();
                move |_, event, window, cx| {
                    open_path(&path, event, window, cx);
                }
            }))
            .context_menu({
                let path = path.clone();
                move |menu, window, cx| {
                    folder_context_menu(menu, &path, false, window, cx)
                }
            })
    }

    fn recent_row(
        &self,
        index: usize,
        item: &RecentItem,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let path = item.path.clone();
        let name = item.label.clone();
        let detail = item.path.display().to_string();
        Button::new(("home-recent-row", index))
            .ghost()
            .w_full()
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_center()
                    .child(shell_icon_for_path(&item.path))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_4()
                            .child(Label::new(name).text_sm().truncate())
                            .child(
                                Label::new(detail)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .flex_1(),
                            ),
                    ),
            )
            .on_click(cx.listener({
                let path = path.clone();
                move |_, event, window, cx| {
                    open_path(&path, event, window, cx);
                }
            }))
            .context_menu({
                let path = path.clone();
                move |menu, window, cx| file_context_menu(menu, &path, window, cx)
            })
    }

    fn tag_container(
        &self,
        index: usize,
        preview: &FileTagPreview,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tag_name = preview.tag.name.clone();
        let view_more = tag_name.clone();
        div()
            .id(("home-tag-container", index))
            .w(CARD_WIDTH)
            .min_h(px(160.))
            .flex_none()
            .rounded(px(6.))
            .border_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .w_full()
                    .child(
                        h_flex()
                            .w_full()
                            .px_2()
                            .py_1()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .child(
                                Button::new(("home-tag-view", index))
                                    .ghost()
                                    .small()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(tag_color_dot(
                                                preview.tag.color.as_deref(),
                                                cx,
                                            ))
                                            .child(
                                                Label::new(tag_name)
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::SEMIBOLD),
                                            ),
                                    )
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        AppNavigation::navigate_to_file_tag(
                                            view_more.clone(),
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .p_1()
                            .gap_px()
                            .when(preview.preview_items.is_empty(), |col| {
                                col.child(
                                    Label::new(t!("home.widget.tags.preview.empty"))
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                )
                            })
                            .children(
                                preview.preview_items.iter().enumerate().map(
                                    |(row, (name, file_path))| {
                                        let open = file_path.clone();
                                        Button::new(SharedString::from(format!(
                                            "home-tag-file-{index}-{row}"
                                        )))
                                            .ghost()
                                            .w_full()
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(shell_icon_for_path(file_path))
                                                    .child(
                                                        Label::new(name.clone())
                                                            .text_sm()
                                                            .truncate(),
                                                    ),
                                            )
                                            .on_click(cx.listener({
                                                let open = open.clone();
                                                move |_, event, window, cx| {
                                                    open_path(&open, event, window, cx);
                                                }
                                            }))
                                            .context_menu({
                                                let open = open.clone();
                                                move |menu, window, cx| {
                                                    file_context_menu(menu, &open, window, cx)
                                                }
                                            })
                                    },
                                ),
                            ),
                    ),
            )
    }
}

fn tag_color_dot(color: Option<&str>, cx: &mut App) -> impl IntoElement {
    let fill = color
        .and_then(parse_hex_color)
        .unwrap_or(cx.theme().primary);
    div().size(px(10.)).rounded_full().bg(fill)
}

fn parse_hex_color(s: &str) -> Option<Hsla> {
    let hex = s.trim().trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(
                gpui::rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
                    .into(),
            )
        }
        _ => None,
    }
}

fn open_path(path: &PathBuf, event: &ClickEvent, _window: &Window, cx: &mut App) {
    if event.modifiers().control {
        AppNavigation::open_path_in_new_tab(path.clone(), cx);
    } else {
        AppNavigation::navigate_to_path(path.clone(), cx);
    }
}

fn folder_context_menu(
    menu: PopupMenu,
    path: &PathBuf,
    is_pinned: bool,
    _window: &mut Window,
    cx: &mut App,
) -> PopupMenu {
    let path_string = path.to_string_lossy().to_string();
    let path_open = path.clone();
    let path_tab = path.clone();
    let path_pin = path.clone();
    let path_unpin = path_string.clone();
    let path_props = path.clone();
    let mut menu = menu.item(
        PopupMenuItem::new(t!("sidebar.menu.open")).on_click(move |_, _, cx| {
            AppNavigation::navigate_to_path(path_open.clone(), cx);
        }),
    );
    menu = menu.item(
        PopupMenuItem::new(t!("sidebar.menu.open_new_tab")).on_click(move |_, _, cx| {
            AppNavigation::open_path_in_new_tab(path_tab.clone(), cx);
        }),
    );
    if is_pinned {
        menu = menu.item(
            PopupMenuItem::new(t!("sidebar.menu.unpin")).on_click(move |_, _, cx| {
                AppNavigation::unpin_folder(&path_unpin, cx);
            }),
        );
    } else {
        menu = menu.item(PopupMenuItem::new(t!("sidebar.menu.pin")).on_click(move |_, _, cx| {
            AppNavigation::pin_folder(path_pin.clone(), cx);
        }));
    }
    menu.item(PopupMenuItem::new(t!("files.menu.properties")).on_click(move |_, _, cx| {
        let _ = open_item_properties(path_props.as_path());
        cx.stop_propagation();
    }))
}

fn file_context_menu(
    menu: PopupMenu,
    path: &PathBuf,
    _window: &mut Window,
    cx: &mut App,
) -> PopupMenu {
    let path_open = path.clone();
    let path_tab = path.clone();
    let path_props = path.clone();
    menu.item(
        PopupMenuItem::new(t!("sidebar.menu.open")).on_click(move |_, _, cx| {
            AppNavigation::navigate_to_path(path_open.clone(), cx);
        }),
    )
    .item(
        PopupMenuItem::new(t!("sidebar.menu.open_new_tab")).on_click(move |_, _, cx| {
            AppNavigation::open_path_in_new_tab(path_tab.clone(), cx);
        }),
    )
    .item(PopupMenuItem::new(t!("files.menu.properties")).on_click(move |_, _, cx| {
        let _ = open_item_properties(path_props.as_path());
        cx.stop_propagation();
    }))
}
