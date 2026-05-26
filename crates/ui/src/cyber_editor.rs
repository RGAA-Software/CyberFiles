use std::path::{Path, PathBuf};

use gpui::{
    actions, div, prelude::FluentBuilder, App, AppContext, ClickEvent, Context, Entity,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, ParentElement, Render,
    SharedString, Styled, Subscription, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState, TabSize},
    label::Label,
    notification::Notification,
    v_flex, ActiveTheme as _, Disableable, Selectable, Sizable as _, StyledExt,
    WindowExt as _,
};

const APP_NAME: &str = "CyberEditor";
const EDITOR_CONTEXT: &str = "CyberEditor";

actions!(cybereditor, [SaveFile, SaveFileAs]);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-s", SaveFile, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-s", SaveFileAs, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-s", SaveFile, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-s", SaveFileAs, Some(EDITOR_CONTEXT)),
    ]);
}

struct LoadedDocument {
    text: String,
    load_error: Option<String>,
}

pub struct CyberEditorPage {
    focus_handle: FocusHandle,
    editor: Entity<InputState>,
    file_path: Option<PathBuf>,
    language: SharedString,
    saved_text: String,
    dirty: bool,
    line_numbers: bool,
    soft_wrap: bool,
    _subscriptions: Vec<Subscription>,
}

impl CyberEditorPage {
    pub fn view(path: Option<PathBuf>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let page = cx.new(|cx| Self::new(path, window, cx));
        let weak = page.downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |page, cx| page.request_close(window, cx))
                .unwrap_or(true)
        });
        page
    }

    pub fn new(path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let document = load_document(path.as_deref());
        let language = SharedString::from(language_for_path(path.as_deref()));
        let initial_text = document.text;
        let line_numbers = true;
        let soft_wrap = false;

        let editor = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(language.clone())
                .line_number(line_numbers)
                .folding(true)
                .indent_guides(true)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                .soft_wrap(soft_wrap)
                .default_value(initial_text.clone())
                .placeholder("Open a UTF-8 text or code file")
        });
        let editor_focus = editor.focus_handle(cx);
        window.defer(cx, move |window, cx| {
            editor_focus.focus(window, cx);
        });

        let editor_for_subscription = editor.clone();
        let subscription = cx.subscribe(&editor, move |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let current_text = editor_for_subscription.read(cx).value().to_string();
                let dirty = current_text != this.saved_text;
                if dirty != this.dirty {
                    this.dirty = dirty;
                    cx.notify();
                }
            }
        });

        if let Some(error) = document.load_error {
            window.push_notification(Notification::error(error), cx);
        }

        Self {
            focus_handle: cx.focus_handle(),
            editor,
            file_path: path,
            language,
            saved_text: initial_text,
            dirty: false,
            line_numbers,
            soft_wrap,
            _subscriptions: vec![subscription],
        }
    }

    fn save(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.save_current(window, cx);
    }

    fn save_as(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.open_save_as_dialog(window, cx);
    }

    fn save_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.file_path.clone() {
            let _ = self.write_to_path(path, window, cx);
        } else {
            self.open_save_as_dialog(window, cx);
        }
    }

    fn write_to_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let text = self.editor.read(cx).value().to_string();
        std::fs::write(&path, text.as_bytes())
            .map_err(|err| format!("Failed to save {}: {err}", path.display()))?;

        self.file_path = Some(path.clone());
        self.saved_text = text;
        self.dirty = false;
        self.language = SharedString::from(language_for_path(Some(&path)));
        self.editor.update(cx, |editor, cx| {
            editor.set_highlighter(self.language.clone(), cx);
        });
        window.push_notification(
            Notification::success(format!("Saved {}", path.display())),
            cx,
        );
        cx.notify();
        Ok(())
    }

    fn open_save_as_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default_path = self.suggested_save_path();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(default_path.to_string_lossy().to_string())
                .placeholder("Enter the destination file path")
        });
        let page = cx.entity().downgrade();

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            window.defer(cx, move |window, cx| {
                input_for_focus.update(cx, |input, cx| {
                    input.focus(window, cx);
                });
            });

            alert
                .title("Save As")
                .description("Enter the full destination path for this document.")
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        window.push_notification(
                            Notification::warning("A destination path is required."),
                            cx,
                        );
                        return false;
                    }
                    let path = PathBuf::from(raw);
                    match page_for_submit.update(cx, |page, cx| {
                        page.write_to_path(path, window, cx)
                    }) {
                        Ok(Ok(())) => true,
                        Ok(Err(message)) => {
                            window.push_notification(Notification::error(message), cx);
                            false
                        }
                        Err(_) => true,
                    }
                })
        });
    }

    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.dirty {
            return true;
        }
        if window.has_active_dialog(cx) {
            return false;
        }

        let page = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title("Unsaved Changes")
                .description("This file has unsaved changes. Save before closing?")
                .show_cancel(true)
                .on_ok({
                    let page = page.clone();
                    move |_, window, cx| match page.update(cx, |page, cx| {
                        page.save_current(window, cx);
                        !page.dirty
                    }) {
                        Ok(true) => {
                            window.remove_window();
                            true
                        }
                        Ok(false) => false,
                        Err(_) => true,
                    }
                })
                .on_cancel({
                    move |_, window, _cx| {
                        window.remove_window();
                        true
                    }
                })
        });
        false
    }

    fn suggested_save_path(&self) -> PathBuf {
        if let Some(path) = self.file_path.clone() {
            return path;
        }

        let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        path.push("untitled.txt");
        path
    }

    fn toggle_line_numbers(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.line_numbers = !self.line_numbers;
        self.editor.update(cx, |editor, cx| {
            editor.set_line_number(self.line_numbers, window, cx);
        });
        cx.notify();
    }

    fn toggle_soft_wrap(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.soft_wrap = !self.soft_wrap;
        self.editor.update(cx, |editor, cx| {
            editor.set_soft_wrap(self.soft_wrap, window, cx);
        });
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_3()
                    .child(
                        Label::new(display_name(self.file_path.as_deref()))
                            .text_sm()
                            .font_semibold()
                            .truncate(),
                    )
                    .child(
                        Label::new(display_language(&self.language))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .when(self.dirty, |row| {
                        row.child(
                            Label::new("Unsaved")
                                .text_xs()
                                .text_color(cx.theme().warning),
                        )
                    }),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("toggle-line-numbers")
                            .small()
                            .ghost()
                            .selected(self.line_numbers)
                            .label("Line Numbers")
                            .on_click(cx.listener(Self::toggle_line_numbers)),
                    )
                    .child(
                        Button::new("toggle-soft-wrap")
                            .small()
                            .ghost()
                            .selected(self.soft_wrap)
                            .label("Wrap")
                            .on_click(cx.listener(Self::toggle_soft_wrap)),
                    )
                    .child(
                        Button::new("save-as")
                            .small()
                            .ghost()
                            .label("Save As")
                            .on_click(cx.listener(Self::save_as)),
                    )
                    .child(
                        Button::new("save")
                            .small()
                            .label("Save")
                            .disabled(!self.dirty && self.file_path.is_some())
                            .on_click(cx.listener(Self::save)),
                    ),
            )
    }

    fn window_title(&self) -> SharedString {
        let prefix = if self.dirty { "* " } else { "" };
        SharedString::from(format!(
            "{prefix}{} - {APP_NAME}",
            display_name(self.file_path.as_deref())
        ))
    }
}

impl Focusable for CyberEditorPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CyberEditorPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.window_title();
        window.set_window_title(&title);

        v_flex()
            .id("cyber-editor-page")
            .size_full()
            .min_h_0()
            .min_w_0()
            .track_focus(&self.focus_handle)
            .key_context(EDITOR_CONTEXT)
            .on_action(cx.listener(|this, _: &SaveFile, window, cx| {
                this.save_current(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveFileAs, window, cx| {
                this.open_save_as_dialog(window, cx);
            }))
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(
                        Input::new(&self.editor)
                            .bordered(false)
                            .focus_bordered(false)
                            .p_0()
                            .h_full()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(cx.theme().mono_font_size),
                    ),
            )
    }
}

fn load_document(path: Option<&Path>) -> LoadedDocument {
    let Some(path) = path else {
        return LoadedDocument {
            text: String::new(),
            load_error: None,
        };
    };

    if !path.exists() {
        return LoadedDocument {
            text: String::new(),
            load_error: None,
        };
    }

    if path.is_dir() {
        return LoadedDocument {
            text: String::new(),
            load_error: Some(format!("{} is a directory, not a file.", path.display())),
        };
    }

    match std::fs::read_to_string(path) {
        Ok(text) => LoadedDocument {
            text,
            load_error: None,
        },
        Err(err) => LoadedDocument {
            text: String::new(),
            load_error: Some(format!(
                "Failed to open {} as UTF-8 text: {err}",
                path.display()
            )),
        },
    }
}

fn display_name(path: Option<&Path>) -> SharedString {
    match path.and_then(|path| path.file_name()).and_then(|name| name.to_str()) {
        Some(name) if !name.is_empty() => SharedString::from(name),
        _ => SharedString::from("Untitled"),
    }
}

fn display_language(language: &SharedString) -> SharedString {
    SharedString::from(format!("Language: {}", language))
}

fn language_for_path(path: Option<&Path>) -> &'static str {
    let Some(ext) = path
        .and_then(|path| path.extension())
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    else {
        return "text";
    };

    match ext.as_str() {
        "rs" => "rust",
        "js" | "cjs" | "mjs" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "javascript",
        "py" => "python",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "sql" => "sql",
        "sh" => "bash",
        "xml" => "xml",
        "c" => "c",
        "cc" | "cpp" | "cxx" | "h" | "hpp" => "cpp",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "rb" => "ruby",
        "php" => "php",
        _ => "text",
    }
}
