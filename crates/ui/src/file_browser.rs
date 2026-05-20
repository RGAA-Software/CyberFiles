use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};
use cyberfiles_fs::{read_directory, DirectoryReadOptions, FileItem, FileItemKind};
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    scroll::ScrollableElement as _,
    v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};

pub struct FileBrowser {
    current_dir: PathBuf,
    back_stack: Vec<PathBuf>,
    forward_stack: Vec<PathBuf>,
    items: Vec<FileItem>,
    error: Option<String>,
    selected_path: Option<PathBuf>,
}

impl FileBrowser {
    pub fn new() -> Self {
        let current_dir = default_files_dir();
        let (items, error) = load_files_dir(&current_dir);

        Self {
            current_dir,
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            items,
            error,
            selected_path: None,
        }
    }

    fn refresh(&mut self) {
        let (items, error) = load_files_dir(&self.current_dir);
        self.items = items;
        self.error = error;
        self.reconcile_selection();
    }

    fn navigate_to(&mut self, path: PathBuf) {
        if path == self.current_dir {
            return;
        }

        self.back_stack.push(self.current_dir.clone());
        self.forward_stack.clear();
        self.current_dir = path;
        self.selected_path = None;
        self.refresh();
    }

    fn navigate_back(&mut self) {
        let Some(path) = self.back_stack.pop() else {
            return;
        };

        self.forward_stack.push(self.current_dir.clone());
        self.current_dir = path;
        self.selected_path = None;
        self.refresh();
    }

    fn navigate_forward(&mut self) {
        let Some(path) = self.forward_stack.pop() else {
            return;
        };

        self.back_stack.push(self.current_dir.clone());
        self.current_dir = path;
        self.selected_path = None;
        self.refresh();
    }

    fn navigate_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.navigate_to(parent.to_path_buf());
        }
    }

    fn select_path(&mut self, path: PathBuf) {
        self.selected_path = Some(path);
    }

    fn open_item(&mut self, path: PathBuf, kind: FileItemKind) {
        match kind {
            FileItemKind::Folder => self.navigate_to(path),
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                if let Err(error) = open_with_system(&path) {
                    self.error = Some(error.to_string());
                }
            }
        }
    }

    fn reconcile_selection(&mut self) {
        if let Some(selected_path) = &self.selected_path {
            if !self.items.iter().any(|item| &item.path == selected_path) {
                self.selected_path = None;
            }
        }
    }

    fn table(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .child(
                h_flex()
                    .h_8()
                    .px_3()
                    .gap_3()
                    .items_center()
                    .bg(cx.theme().muted)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().w(px(28.)).flex_none())
                    .child(div().flex_1().min_w_0().child("Name"))
                    .child(div().w(px(110.)).child("Type"))
                    .child(div().w(px(100.)).child("Size"))
                    .child(div().w(px(150.)).child("Modified"))
                    .child(div().w(px(40.)).flex_none()),
            )
            .child(
                v_flex().flex_1().min_h_0().overflow_y_scrollbar().children(
                    self.items
                        .iter()
                        .enumerate()
                        .map(|(index, item)| self.row(index, item, cx)),
                ),
            )
    }

    fn row(&self, index: usize, item: &FileItem, cx: &Context<Self>) -> AnyElement {
        let selected = self.selected_path.as_ref() == Some(&item.path);
        let selected_path = item.path.clone();
        let open_path = item.path.clone();
        let kind = item.kind;
        let icon = match item.kind {
            FileItemKind::Folder => IconName::Folder,
            FileItemKind::Symlink => IconName::ExternalLink,
            FileItemKind::File | FileItemKind::Other => IconName::File,
        };

        h_flex()
            .id(("file-row", index))
            .h_9()
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_path(selected_path.clone());
                cx.notify();
            }))
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(icon).small()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(item.display_name.clone()),
            )
            .child(
                div()
                    .w(px(110.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(item_type_label(item)),
            )
            .child(
                div()
                    .w(px(100.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_size(item.size)),
            )
            .child(
                div()
                    .w(px(150.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_system_time(item.modified)),
            )
            .child(
                div().w(px(40.)).flex_none().child(
                    Button::new(format!("open-item-{index}"))
                        .xsmall()
                        .ghost()
                        .icon(match kind {
                            FileItemKind::Folder => IconName::ChevronRight,
                            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                                IconName::ExternalLink
                            }
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_item(open_path.clone(), kind);
                            cx.notify();
                        })),
                ),
            )
            .into_any_element()
    }
}

impl Render for FileBrowser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_dir = self.current_dir.to_string_lossy().to_string();
        let can_go_back = !self.back_stack.is_empty();
        let can_go_forward = !self.forward_stack.is_empty();
        let can_go_up = self.current_dir.parent().is_some();
        let selected_count = usize::from(self.selected_path.is_some());

        v_flex()
            .id("files-page")
            .size_full()
            .min_h_0()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("files-back")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowLeft)
                            .disabled(!can_go_back)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_back();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("files-forward")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowRight)
                            .disabled(!can_go_forward)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_forward();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("files-up")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowUp)
                            .disabled(!can_go_up)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate_parent();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("files-refresh")
                            .small()
                            .ghost()
                            .icon(IconName::Redo2)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .px_3()
                            .py_1()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .text_color(cx.theme().muted_foreground)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(current_dir),
                    ),
            )
            .when_some(self.error.as_ref(), |this, error| {
                this.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().danger)
                        .text_color(cx.theme().danger)
                        .child(error.clone()),
                )
            })
            .child(self.table(cx))
            .child(
                h_flex()
                    .justify_between()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} items, {} selected",
                        self.items.len(),
                        selected_count
                    ))
                    .child("Local filesystem"),
            )
    }
}

fn default_files_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn load_files_dir(path: &Path) -> (Vec<FileItem>, Option<String>) {
    match read_directory(path, DirectoryReadOptions::default()) {
        Ok(items) => (items, None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    }
}

fn item_type_label(item: &FileItem) -> String {
    match item.kind {
        FileItemKind::Folder => "Folder".to_string(),
        FileItemKind::Symlink => "Symlink".to_string(),
        FileItemKind::Other => "Item".to_string(),
        FileItemKind::File => item
            .extension
            .as_ref()
            .map(|extension| format!("{} file", extension.to_uppercase()))
            .unwrap_or_else(|| "File".to_string()),
    }
}

fn format_size(size: Option<u64>) -> String {
    let Some(size) = size else {
        return String::new();
    };

    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;

    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", size, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_system_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return String::new();
    };

    let local_time: DateTime<Local> = time.into();
    local_time.format("%Y-%m-%d %H:%M").to_string()
}

fn open_with_system(path: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }
}
