use std::{cell::Cell, path::PathBuf, rc::Rc};

use gpui::{
    div, prelude::FluentBuilder, App, AppContext, ClickEvent, Context, Entity, FocusHandle,
    Focusable, InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    notification::Notification,
    v_flex, ActiveTheme as _, Disableable, Selectable, Sizable as _, StyledExt,
    WindowExt as _,
};

use crate::title_bar::TitleBar;

use super::{
    display_language, display_name, display_path, load_document, EditorHost, EditorSession,
    FindNext, FindPrevious, FindText, GoToLine, OpenFile, ReplaceText, SaveFile, SaveFileAs,
    SearchMatch, APP_NAME, EDITOR_CONTEXT,
};

pub struct CyberEditorPage {
    focus_handle: FocusHandle,
    editor: EditorHost,
    session: EditorSession,
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
        let initial_text = document.text;
        let session = EditorSession::new(path, initial_text.clone());

        let editor = EditorHost::new(
            window,
            cx,
            session.language().clone(),
            initial_text.clone(),
            session.line_numbers(),
            session.soft_wrap(),
        );
        editor.focus_deferred(window, cx);

        let editor_for_subscription = editor.entity().clone();
        let subscription = cx.subscribe(editor.entity(), move |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let editor_state = editor_for_subscription.read(cx);
                let current_text = editor_state.value().to_string();
                this.editor.sync_text_change(&current_text);
                this.editor.sync_cursor_position(editor_state.cursor_position());
                if this.session.update_dirty_from_text(&current_text) {
                    cx.notify();
                } else {
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
            session,
            _subscriptions: vec![subscription],
        }
    }

    fn save(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.save_current(window, cx);
    }

    fn open_file(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.open_file_dialog(window, cx);
    }

    fn save_as(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.open_save_as_dialog(window, cx);
    }

    fn save_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.session.file_path().cloned() {
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
        let text = self.editor.text(cx);
        std::fs::write(&path, text.as_bytes())
            .map_err(|err| format!("Failed to save {}: {err}", path.display()))?;

        self.session.apply_save(path.clone(), text);
        self.editor
            .set_highlighter(self.session.language().clone(), cx);
        window.push_notification(
            Notification::success(format!("Saved {}", path.display())),
            cx,
        );
        cx.notify();
        Ok(())
    }

    fn load_path_into_editor(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let document = load_document(Some(&path));
        if let Some(error) = document.load_error {
            return Err(error);
        }

        let text = document.text;
        self.editor
            .set_document(
                text.clone(),
                SharedString::from(super::language_for_path(Some(&path))),
                window,
                cx,
            );
        self.session.apply_loaded_document(path, text);
        cx.notify();
        Ok(())
    }

    fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default_path = self
            .session
            .file_path()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(default_path.to_string_lossy().to_string())
                .placeholder("Enter the full file path to open")
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

            alert
                .title("Open File")
                .description("Enter the full path of the UTF-8 text or code file to open.")
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        window.push_notification(
                            Notification::warning("A file path is required."),
                            cx,
                        );
                        return false;
                    }
                    let path = PathBuf::from(raw);
                    match page_for_submit.update(cx, |page, cx| {
                        page.load_path_into_editor(path, window, cx)
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

    fn open_save_as_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default_path = self.suggested_save_path();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(default_path.to_string_lossy().to_string())
                .placeholder("Enter the destination file path")
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

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

    fn go_to_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.editor.cursor_position();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(format!("{}:{}", cursor.line + 1, cursor.character + 1))
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

            alert
                .title("Go to Line")
                .description("Enter a 1-based line or line:column target.")
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    let Some(position) = parse_go_to_line_target(&raw) else {
                        window.push_notification(
                            Notification::warning("Enter a line number or line:column."),
                            cx,
                        );
                        return false;
                    };
                    match page_for_submit.update(cx, |page, cx| {
                        page.editor.set_cursor_position(position, window, cx);
                        cx.notify();
                    }) {
                        Ok(_) => true,
                        Err(_) => true,
                    }
                })
        });
    }

    fn open_find_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.session.find_query().to_string())
                .placeholder("Enter text to find")
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

            alert
                .title("Find")
                .description("Enter text to find from the current cursor position.")
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        window.push_notification(
                            Notification::warning("Enter text to find."),
                            cx,
                        );
                        return false;
                    }
                    match page_for_submit.update(cx, |page, cx| {
                        page.find_next(&raw, window, cx)
                    }) {
                        Ok(found) => found,
                        Err(_) => true,
                    }
                })
        });
    }

    fn open_replace_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let find_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.session.find_query().to_string())
                .placeholder("Find")
        });
        let replace_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.session.replace_query().to_string())
                .placeholder("Replace with")
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let find_input_for_focus = find_input.clone();
            let find_input_for_submit = find_input.clone();
            let replace_input_for_submit = replace_input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, find_input_for_focus, window, cx);

            alert
                .title("Replace")
                .description("Replace the next match from the current cursor position.")
                .show_cancel(true)
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(Input::new(&find_input).w_full())
                        .child(Input::new(&replace_input).w_full()),
                )
                .on_ok(move |_, window, cx| {
                    let find = find_input_for_submit.read(cx).value().trim().to_string();
                    let replace_with = replace_input_for_submit.read(cx).value().to_string();
                    if find.is_empty() {
                        window.push_notification(
                            Notification::warning("Enter text to find."),
                            cx,
                        );
                        return false;
                    }
                    match page_for_submit.update(cx, |page, cx| {
                        page.replace_next(&find, &replace_with, window, cx)
                    }) {
                        Ok(replaced) => replaced,
                        Err(_) => true,
                    }
                })
        });
    }

    fn find_next(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.session.set_find_query(query.to_string());
        let Some(search_match) = self.editor.find_next(query) else {
            window.push_notification(
                Notification::warning(format!("No match found for \"{query}\".")),
                cx,
            );
            cx.notify();
            return false;
        };

        self.schedule_select_match(search_match, cx);
        true
    }

    fn find_next_from_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.session.find_query().trim().to_string();
        if query.is_empty() {
            self.open_find_dialog(window, cx);
            return;
        }

        let _ = self.find_next(&query, window, cx);
    }

    fn find_previous(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.session.find_query().trim().to_string();
        if query.is_empty() {
            self.open_find_dialog(window, cx);
            return;
        }

        let Some(search_match) = self.editor.find_previous(&query) else {
            window.push_notification(
                Notification::warning(format!("No match found for \"{query}\".")),
                cx,
            );
            cx.notify();
            return;
        };

        self.schedule_select_match(search_match, cx);
    }

    fn replace_next(
        &mut self,
        query: &str,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.session.set_find_query(query.to_string());
        self.session.set_replace_query(replacement.to_string());

        let current_text = self.editor.text(cx);
        let cursor = self.editor.cursor_position();
        let Some((new_text, replacement_match)) =
            replace_next_in_text(&current_text, cursor, query, replacement)
        else {
            window.push_notification(
                Notification::warning(format!("No match found for \"{query}\".")),
                cx,
            );
            cx.notify();
            return false;
        };

        self.editor
            .set_document(new_text.clone(), self.session.language().clone(), window, cx);
        self.session.update_dirty_from_text(&new_text);
        self.schedule_select_match(replacement_match, cx);
        cx.notify();
        true
    }

    fn schedule_select_match(&mut self, search_match: SearchMatch, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        cx.defer(move |cx| {
            let Some(window) = cx.active_window() else {
                return;
            };
            let _ = window.update(cx, |_, window, cx| {
                editor.select_match(search_match, window, cx);
            });
        });
    }

    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.session.dirty() {
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
                        !page.session.dirty()
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
        self.session.suggested_save_path()
    }

    fn toggle_line_numbers(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let line_numbers = self.session.toggle_line_numbers();
        self.editor.set_line_numbers(line_numbers, window, cx);
        cx.notify();
    }

    fn toggle_soft_wrap(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let soft_wrap = self.session.toggle_soft_wrap();
        self.editor.set_soft_wrap(soft_wrap, window, cx);
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
                        Label::new(display_name(self.session.file_path().map(PathBuf::as_path)))
                            .text_sm()
                            .font_semibold()
                            .truncate(),
                    )
                    .child(
                        Label::new(display_language(self.session.language()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .when(self.session.dirty(), |row| {
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
                        Button::new("open-file")
                            .small()
                            .ghost()
                            .label("Open File")
                            .on_click(cx.listener(Self::open_file)),
                    )
                    .child(
                        Button::new("toggle-line-numbers")
                            .small()
                            .ghost()
                            .selected(self.session.line_numbers())
                            .label("Line Numbers")
                            .on_click(cx.listener(Self::toggle_line_numbers)),
                    )
                    .child(
                        Button::new("toggle-soft-wrap")
                            .small()
                            .ghost()
                            .selected(self.session.soft_wrap())
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
                        Button::new("go-to-line")
                            .small()
                            .ghost()
                            .label("Go to Line")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.go_to_line(window, cx);
                            })),
                    )
                    .child(
                        Button::new("find-text")
                            .small()
                            .ghost()
                            .label("Find")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_find_dialog(window, cx);
                            })),
                    )
                    .child(
                        Button::new("replace-text")
                            .small()
                            .ghost()
                            .label("Replace")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_replace_dialog(window, cx);
                            })),
                    )
                    .child(
                        Button::new("save")
                            .small()
                            .label("Save")
                            .disabled(!self.session.dirty() && self.session.file_path().is_some())
                            .on_click(cx.listener(Self::save)),
                    ),
            )
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new().child(
            h_flex()
                .id("cybereditor-title-bar")
                .h_full()
                .w_full()
                .min_w_0()
                .items_center()
                .justify_between()
                .gap_3()
                .px_3()
                .child(
                    h_flex()
                        .min_w_0()
                        .items_center()
                        .gap_3()
                        .child(Label::new(APP_NAME).text_sm().font_semibold())
                        .child(
                            Label::new(display_path(self.session.file_path().map(PathBuf::as_path)))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .truncate(),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Label::new(if self.session.dirty() { "Unsaved" } else { "Saved" })
                                .text_xs()
                                .text_color(if self.session.dirty() {
                                    cx.theme().warning
                                } else {
                                    cx.theme().muted_foreground
                                }),
                        ),
                ),
        )
    }

    fn window_title(&self) -> SharedString {
        let prefix = if self.session.dirty() { "* " } else { "" };
        SharedString::from(format!(
            "{prefix}{} - {APP_NAME}",
            display_name(self.session.file_path().map(PathBuf::as_path))
        ))
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py_1()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().title_bar)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Label::new(if self.session.dirty() { "Modified" } else { "Saved" })
                            .text_xs()
                            .text_color(if self.session.dirty() {
                                cx.theme().warning
                            } else {
                                cx.theme().muted_foreground
                            }),
                    )
                    .child(
                        Label::new(display_language(self.session.language()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Label::new(self.session.encoding_label().clone())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(format!("Lines: {}", self.editor.line_count()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(format!("Chars: {}", self.editor.char_count()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(format!("Rev: {}", self.editor.revision()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Button::new("go-to-line-status")
                            .ghost()
                            .xsmall()
                            .label(format!(
                                "Ln {}, Col {}",
                                self.editor.cursor_position().line + 1,
                                self.editor.cursor_position().character + 1
                            ))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.go_to_line(window, cx);
                            })),
                    )
                    .child(
                        Label::new(self.session.line_ending_label())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(self.session.indent_label())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(if self.session.soft_wrap() { "Wrap On" } else { "Wrap Off" })
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
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
            .child(self.render_title_bar(cx))
            .on_action(cx.listener(|this, _: &SaveFile, window, cx| {
                this.save_current(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFile, window, cx| {
                this.open_file_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveFileAs, window, cx| {
                this.open_save_as_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToLine, window, cx| {
                this.go_to_line(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindText, window, cx| {
                this.open_find_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReplaceText, window, cx| {
                this.open_replace_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindNext, window, cx| {
                this.find_next_from_session(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindPrevious, window, cx| {
                this.find_previous(window, cx);
            }))
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.editor.render(cx)),
            )
            .child(self.render_status_bar(cx))
    }
}

fn parse_go_to_line_target(raw: &str) -> Option<gpui_component::input::Position> {
    let mut parts = raw.split(':');
    let line = parts.next()?.trim().parse::<u32>().ok()?;
    if line == 0 {
        return None;
    }

    let column = match parts.next() {
        Some(value) if !value.trim().is_empty() => value.trim().parse::<u32>().ok()?,
        Some(_) | None => 1,
    };
    if column == 0 || parts.next().is_some() {
        return None;
    }

    Some(gpui_component::input::Position::new(line - 1, column - 1))
}

fn replace_next_in_text(
    text: &str,
    cursor: gpui_component::input::Position,
    query: &str,
    replacement: &str,
) -> Option<(String, SearchMatch)> {
    if query.is_empty() {
        return None;
    }

    let start = position_to_byte_offset(text, cursor);
    let match_offset = text[start..]
        .find(query)
        .map(|offset| start + offset)
        .or_else(|| text[..start].find(query))?;

    let match_end = match_offset + query.len();
    let mut new_text =
        String::with_capacity(text.len() + replacement.len().saturating_sub(query.len()));
    new_text.push_str(&text[..match_offset]);
    new_text.push_str(replacement);
    new_text.push_str(&text[match_end..]);

    Some((
        new_text,
        SearchMatch {
            start: byte_offset_to_position(text, match_offset),
            char_len: replacement.chars().count() as u32,
        },
    ))
}

fn position_to_byte_offset(text: &str, position: gpui_component::input::Position) -> usize {
    let mut line = 0u32;
    let mut column = 0u32;

    for (offset, ch) in text.char_indices() {
        if line == position.line && column == position.character {
            return offset;
        }

        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    text.len()
}

fn byte_offset_to_position(text: &str, byte_offset: usize) -> gpui_component::input::Position {
    let mut line = 0u32;
    let mut column = 0u32;

    for (offset, ch) in text.char_indices() {
        if offset >= byte_offset {
            break;
        }

        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    gpui_component::input::Position::new(line, column)
}

fn focus_input_once(
    armed: &Rc<Cell<bool>>,
    input: Entity<InputState>,
    window: &mut Window,
    cx: &mut App,
) {
    if armed.replace(true) {
        return;
    }

    window.defer(cx, move |window, cx| {
        let _ = input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    });
}
