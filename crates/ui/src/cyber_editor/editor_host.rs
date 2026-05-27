use gpui::{App, AppContext, Context, Entity, Window};
use gpui_component::input::InputState;

use super::{ModelEditorBackend, SearchMatch};

/// Temporary editor host boundary.
/// The first backend still uses `InputState`, but page-level code should talk
/// to this wrapper so the editing core can be replaced later.
#[derive(Clone)]
pub(crate) struct EditorHost {
    backend: ModelEditorBackend,
}

impl EditorHost {
    pub(crate) fn new<T>(
        window: &mut Window,
        cx: &mut Context<T>,
        language: gpui::SharedString,
        initial_text: String,
        line_numbers: bool,
        soft_wrap: bool,
    ) -> Self {
        let backend = ModelEditorBackend::new(
            window,
            cx,
            language,
            initial_text,
            line_numbers,
            soft_wrap,
        );
        Self { backend }
    }

    pub(crate) fn entity(&self) -> &Entity<InputState> {
        self.backend.input_entity()
    }

    pub(crate) fn focus_deferred<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        self.backend.focus_deferred(window, cx);
    }

    pub(crate) fn text(&self, cx: &App) -> String {
        self.backend.text(cx)
    }

    pub(crate) fn set_document(
        &self,
        text: String,
        language: gpui::SharedString,
        window: &mut Window,
        cx: &mut (impl AppContext + std::borrow::BorrowMut<App>),
    ) {
        self.backend.set_document(text, language, window, cx);
    }

    pub(crate) fn set_highlighter<T>(
        &self,
        language: gpui::SharedString,
        cx: &mut Context<T>,
    ) {
        self.backend.set_highlighter(language, cx);
    }

    pub(crate) fn set_line_numbers<T>(
        &self,
        line_numbers: bool,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.backend.set_line_numbers(line_numbers, window, cx);
    }

    pub(crate) fn set_soft_wrap<T>(
        &self,
        soft_wrap: bool,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.backend.set_soft_wrap(soft_wrap, window, cx);
    }

    pub(crate) fn render<T>(&self, cx: &mut Context<T>) -> impl gpui::IntoElement {
        self.backend.render(cx)
    }

    pub(crate) fn sync_text_change(&self, text: &str) -> bool {
        self.backend.sync_text_change(text)
    }

    pub(crate) fn sync_cursor_position(&self, cursor: gpui_component::input::Position) -> bool {
        self.backend.sync_cursor_position(cursor)
    }

    pub(crate) fn sync_selection(
        &self,
        selected_range: std::ops::Range<usize>,
        selected_char_count: usize,
    ) -> bool {
        self.backend
            .sync_selection(selected_range, selected_char_count)
    }

    pub(crate) fn set_cursor_position(
        &self,
        cursor: gpui_component::input::Position,
        window: &mut Window,
        cx: &mut (impl AppContext + std::borrow::BorrowMut<App>),
    ) {
        self.backend.set_cursor_position(cursor, window, cx);
    }

    pub(crate) fn line_count(&self) -> usize {
        self.backend.line_count()
    }

    pub(crate) fn char_count(&self) -> usize {
        self.backend.char_count()
    }

    pub(crate) fn revision(&self) -> u64 {
        self.backend.revision()
    }

    pub(crate) fn cursor_position(&self) -> gpui_component::input::Position {
        self.backend.cursor_position()
    }

    pub(crate) fn selected_char_count(&self) -> usize {
        self.backend.selected_char_count()
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.backend.has_selection()
    }

    pub(crate) fn find_next(&self, query: &str) -> Option<SearchMatch> {
        self.backend.find_next(query)
    }

    pub(crate) fn find_previous(&self, query: &str) -> Option<SearchMatch> {
        self.backend.find_previous(query)
    }

    pub(crate) fn match_count(&self, query: &str) -> usize {
        self.backend.match_count(query)
    }

    pub(crate) fn current_match_index(&self, query: &str) -> usize {
        self.backend.current_match_index(query)
    }

    pub(crate) fn select_match(
        &self,
        search_match: SearchMatch,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.backend.select_match(search_match, window, cx);
    }
}
