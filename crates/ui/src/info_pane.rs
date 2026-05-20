use cyberfiles_fs::FileItem;
use gpui::{prelude::*, *};
use gpui_component::{
    h_flex,
    tab::{Tab, TabBar},
    v_flex, ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
};
use rust_i18n::t;

pub struct InfoPane;

impl InfoPane {
    pub fn render(item: Option<&FileItem>, cx: &App) -> impl IntoElement {
        let (name, detail_lines) = item_details(item);

        v_flex()
            .id("info-pane")
            .size_full()
            .min_w_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                TabBar::new("info-pane-tabs")
                    .small()
                    .selected_index(0)
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
                            .when_some(name.clone(), |panel, name| {
                                panel
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(IconName::Info).small())
                                            .child(div().text_sm().font_medium().child(name)),
                                    )
                                    .children(detail_lines.into_iter().map(|line| {
                                        detail_row(cx, &line.0, &line.1)
                                    }))
                            })
                            .when(name.is_none(), |panel| {
                                panel.child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(t!("info_pane.empty")),
                                )
                            }),
            )
    }
}

fn detail_row(cx: &App, label: &str, value: &str) -> impl IntoElement {
    v_flex()
        .gap_0()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_sm()
                .overflow_hidden()
                .text_ellipsis()
                .child(value.to_string()),
        )
}

fn item_details(item: Option<&FileItem>) -> (Option<String>, Vec<(String, String)>) {
    let Some(item) = item else {
        return (None, Vec::new());
    };

    let name = item.display_name.clone();
    let mut lines = vec![
        (t!("info_pane.path").to_string(), item.path.display().to_string()),
        (t!("info_pane.type").to_string(), item_type_label(item)),
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
    use cyberfiles_fs::FileItemKind;
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
