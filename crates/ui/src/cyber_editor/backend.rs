use std::{cell::RefCell, rc::Rc};

use gpui::{App, AppContext, Context, Entity, Focusable, Keystroke, Styled, Window};
use gpui_component::{
    input::{Input, InputState, TabSize},
    ActiveTheme as _,
};

use super::{EditorBufferModel, SearchMatch};

#[derive(Clone)]
pub(crate) struct ModelEditorBackend {
    input: Entity<InputState>,
    buffer: Rc<RefCell<EditorBufferModel>>,
}

impl ModelEditorBackend {
    pub(super) fn input_entity(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) fn new<T>(
        window: &mut Window,
        cx: &mut Context<T>,
        language: gpui::SharedString,
        initial_text: String,
        line_numbers: bool,
        soft_wrap: bool,
    ) -> Self {
        let input = cx.new(|cx| {
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

        let buffer = Rc::new(RefCell::new(EditorBufferModel::new(initial_text, language)));
        Self { input, buffer }
    }

    pub(super) fn focus_deferred<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        let editor_focus = self.input.focus_handle(cx);
        window.defer(cx, move |window, cx| {
            editor_focus.focus(window, cx);
        });
    }

    pub(super) fn text(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }

    pub(super) fn set_document<T>(
        &self,
        text: String,
        language: gpui::SharedString,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.buffer
            .borrow_mut()
            .set_document(text.clone(), language.clone());
        self.input.update(cx, |editor, cx| {
            editor.set_value(text, window, cx);
            editor.set_highlighter(language, cx);
        });
    }

    pub(super) fn set_highlighter<T>(&self, language: gpui::SharedString, cx: &mut Context<T>) {
        self.buffer.borrow_mut().set_language(language.clone());
        self.input.update(cx, |editor, cx| {
            editor.set_highlighter(language, cx);
        });
    }

    pub(super) fn set_line_numbers<T>(
        &self,
        line_numbers: bool,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.input.update(cx, |editor, cx| {
            editor.set_line_number(line_numbers, window, cx);
        });
    }

    pub(super) fn set_soft_wrap<T>(
        &self,
        soft_wrap: bool,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.input.update(cx, |editor, cx| {
            editor.set_soft_wrap(soft_wrap, window, cx);
        });
    }

    pub(super) fn sync_text_change(&self, text: &str) {
        self.buffer.borrow_mut().sync_text(text);
    }

    pub(super) fn sync_cursor_position(&self, cursor: gpui_component::input::Position) {
        self.buffer.borrow_mut().sync_cursor(cursor);
    }

    pub(super) fn set_cursor_position<T>(
        &self,
        position: gpui_component::input::Position,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.buffer.borrow_mut().sync_cursor(position);
        self.input.update(cx, |editor, cx| {
            editor.set_cursor_position(position, window, cx);
        });
    }

    pub(super) fn line_count(&self) -> usize {
        self.buffer.borrow().line_count()
    }

    pub(super) fn char_count(&self) -> usize {
        self.buffer.borrow().char_count()
    }

    pub(super) fn revision(&self) -> u64 {
        self.buffer.borrow().revision()
    }

    pub(super) fn cursor_position(&self) -> gpui_component::input::Position {
        self.buffer.borrow().cursor()
    }

    pub(super) fn find_next(&self, query: &str) -> Option<SearchMatch> {
        self.buffer.borrow().find_next(query)
    }

    pub(super) fn find_previous(&self, query: &str) -> Option<SearchMatch> {
        self.buffer.borrow().find_previous(query)
    }

    pub(super) fn select_match(
        &self,
        search_match: SearchMatch,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.buffer.borrow_mut().sync_cursor(search_match.start);
        self.input.update(cx, |editor, cx| {
            editor.set_cursor_position(search_match.start, window, cx);
        });
        for _ in 0..search_match.char_len {
            let _ = window.dispatch_keystroke(Keystroke::parse("shift-right").unwrap(), cx);
        }
    }

    pub(super) fn render<T>(&self, cx: &mut Context<T>) -> impl gpui::IntoElement {
        Input::new(&self.input)
            .bordered(false)
            .focus_bordered(false)
            .p_0()
            .h_full()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(cx.theme().mono_font_size)
    }
}
