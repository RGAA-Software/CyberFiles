use cyberfiles_fs::{
    is_image_path, is_text_preview_path, read_text_preview, FileItem, FileItemKind,
};
use gpui::{img, prelude::*, ObjectFit, *};
use gpui_component::{
    alert::Alert,
    description_list::{DescriptionItem, DescriptionList},
    h_flex,
    label::Label,
    tab::{Tab, TabBar},
    v_flex, ActiveTheme as _, IconName,
};
use rust_i18n::t;

use crate::icons::icon_foreground;

pub struct InfoPane {
    selected_tab: usize,
    item: Option<FileItem>,
}

impl InfoPane {
    pub fn new() -> Self {
        Self {
            selected_tab: 0,
            item: None,
        }
    }

    pub fn set_item(&mut self, item: Option<FileItem>) {
        self.item = item;
    }
}

impl Render for InfoPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_tab = self.selected_tab;
        let item = self.item.clone();

        v_flex()
            .id("info-pane")
            .size_full()
            .min_w_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                TabBar::new("info-pane-tabs")
                    .w_full()
                    .selected_index(selected_tab)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.selected_tab = *ix;
                        cx.notify();
                    }))
                    .child(Tab::new().label(t!("info_pane.tab.details")))
                    .child(Tab::new().label(t!("info_pane.tab.preview"))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .p_3()
                    .gap_3()
                    .child(tab_content(selected_tab, item.as_ref(), cx)),
            )
    }
}

fn tab_content(
    selected_tab: usize,
    item: Option<&FileItem>,
    cx: &mut Context<InfoPane>,
) -> AnyElement {
    if selected_tab == 0 {
        details_panel(item, cx).into_any_element()
    } else {
        preview_panel(item, cx).into_any_element()
    }
}

fn details_panel(item: Option<&FileItem>, cx: &mut Context<InfoPane>) -> impl IntoElement {
    let (name, detail_lines) = item_details(item);

    v_flex()
        .w_full()
        .gap_3()
        .when_some(name.clone(), |panel, name| {
            panel
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_color(cx.theme().foreground)
                        .child(icon_foreground(IconName::Info, cx))
                        .child(
                            Label::new(name)
                                .text_sm()
                                .text_color(cx.theme().foreground),
                        ),
                )
                .child(
                    DescriptionList::vertical()
                        .bordered(false)
                        .columns(1)
                        .children(detail_lines.into_iter().map(|(label, value)| {
                            DescriptionItem::new(label).value(value)
                        })),
                )
        })
        .when(name.is_none(), |panel| {
            panel.child(Alert::info(
                "info-pane-empty",
                t!("info_pane.empty").to_string(),
            ))
        })
}

fn preview_panel(item: Option<&FileItem>, cx: &mut Context<InfoPane>) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2()
        .when(item.is_none(), |panel| panel.child(empty_preview()))
        .when_some(item.as_ref(), |panel, item| {
            if item.kind == FileItemKind::Folder {
                panel.child(Alert::info(
                    "info-pane-preview-folder",
                    t!("info_pane.preview.folder").to_string(),
                ))
            } else if is_image_path(&item.path) {
                panel.child(
                    img(item.path.clone())
                        .w_full()
                        .max_h(px(360.))
                        .object_fit(ObjectFit::Contain),
                )
            } else if is_text_preview_path(&item.path) {
                panel.child(preview_text_content(&item.path, cx))
            } else {
                panel.child(Alert::warning(
                    "info-pane-preview-unsupported",
                    t!("info_pane.preview.unsupported").to_string(),
                ))
            }
        })
}

fn preview_text_content(path: &std::path::Path, cx: &mut Context<InfoPane>) -> AnyElement {
    match read_text_preview(path) {
        Ok(text) => v_flex()
            .w_full()
            .p_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(
                Label::new(text)
                    .text_xs()
                    .text_color(cx.theme().foreground),
            )
            .into_any_element(),
        Err(error) => Alert::error(
            "info-pane-preview-error",
            format!("{}: {error}", t!("info_pane.preview.error")),
        )
        .into_any_element(),
    }
}

fn empty_preview() -> Alert {
    Alert::info(
        "info-pane-preview-empty",
        t!("info_pane.preview.empty").to_string(),
    )
}

fn item_details(item: Option<&FileItem>) -> (Option<String>, Vec<(String, String)>) {
    let Some(item) = item else {
        return (None, Vec::new());
    };

    let name = item.display_name.clone();
    let mut lines = vec![
        (
            t!("info_pane.path").to_string(),
            item.path.display().to_string(),
        ),
        (
            t!("info_pane.type").to_string(),
            item_type_label(item),
        ),
    ];

    if let Some(size) = item.size {
        lines.push((t!("info_pane.size").to_string(), format_size(size)));
    }
    if let Some(modified) = item.modified {
        lines.push((
            t!("info_pane.modified").to_string(),
            format_system_time(modified),
        ));
    }

    (Some(name), lines)
}

fn item_type_label(item: &FileItem) -> String {
    match item.kind {
        FileItemKind::Folder => t!("files.type.folder").to_string(),
        FileItemKind::Symlink => t!("files.type.symlink").to_string(),
        FileItemKind::Other => t!("files.type.other").to_string(),
        FileItemKind::File => item
            .extension
            .as_ref()
            .map(|e| format!("{} file", e.to_uppercase()))
            .unwrap_or_else(|| t!("files.type.file").to_string()),
    }
}

fn format_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_system_time(time: std::time::SystemTime) -> String {
    use chrono::{DateTime, Local};
    let local_time: DateTime<Local> = time.into();
    local_time.format("%Y-%m-%d %H:%M").to_string()
}
