use gpui::SharedString;
use gpui_component::input::Position;

#[derive(Clone, Copy)]
pub(crate) struct SearchMatch {
    pub(crate) start: Position,
    pub(crate) char_len: u32,
}

pub(crate) struct EditorBufferModel {
    text: String,
    language: SharedString,
    revision: u64,
    line_count: usize,
    char_count: usize,
    cursor: Position,
}

impl EditorBufferModel {
    pub(crate) fn new(text: String, language: SharedString) -> Self {
        let mut this = Self {
            text,
            language,
            revision: 0,
            line_count: 1,
            char_count: 0,
            cursor: Position::new(0, 0),
        };
        this.recompute_metrics();
        this
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn line_count(&self) -> usize {
        self.line_count
    }

    pub(crate) fn char_count(&self) -> usize {
        self.char_count
    }

    pub(crate) fn cursor(&self) -> Position {
        self.cursor
    }

    pub(crate) fn find_next(&self, query: &str) -> Option<SearchMatch> {
        if query.is_empty() {
            return None;
        }

        let start = position_to_byte_offset(&self.text, self.cursor);
        let char_len = query.chars().count() as u32;
        self.text[start..]
            .find(query)
            .map(|offset| start + offset)
            .or_else(|| self.text[..start].find(query))
            .map(|offset| SearchMatch {
                start: byte_offset_to_position(&self.text, offset),
                char_len,
            })
    }

    pub(crate) fn find_previous(&self, query: &str) -> Option<SearchMatch> {
        if query.is_empty() {
            return None;
        }

        let start = position_to_byte_offset(&self.text, self.cursor);
        let char_len = query.chars().count() as u32;
        self.text[..start]
            .rfind(query)
            .or_else(|| self.text[start..].rfind(query).map(|offset| start + offset))
            .map(|offset| SearchMatch {
                start: byte_offset_to_position(&self.text, offset),
                char_len,
            })
    }

    pub(crate) fn set_document(&mut self, text: String, language: SharedString) {
        self.text = text;
        self.language = language;
        self.revision = self.revision.wrapping_add(1);
        self.cursor = Position::new(0, 0);
        self.recompute_metrics();
    }

    pub(crate) fn set_language(&mut self, language: SharedString) {
        self.language = language;
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn sync_text(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        self.text.clear();
        self.text.push_str(text);
        self.revision = self.revision.wrapping_add(1);
        self.recompute_metrics();
    }

    pub(crate) fn sync_cursor(&mut self, cursor: Position) {
        self.cursor = cursor;
    }

    fn recompute_metrics(&mut self) {
        self.char_count = self.text.chars().count();
        self.line_count = self.text.lines().count().max(1);
    }
}

fn position_to_byte_offset(text: &str, position: Position) -> usize {
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

fn byte_offset_to_position(text: &str, byte_offset: usize) -> Position {
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

    Position::new(line, column)
}
