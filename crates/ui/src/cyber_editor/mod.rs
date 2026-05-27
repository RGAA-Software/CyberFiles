mod buffer_model;
mod backend;
mod document;
mod editor_host;
mod language;
mod metadata;
mod page;
mod session;

use gpui::{actions, App, KeyBinding};

pub use page::CyberEditorPage;

pub(crate) use buffer_model::{EditorBufferModel, SearchMatch};
pub(crate) use backend::ModelEditorBackend;
pub(crate) use document::{display_language, display_name, display_path, load_document};
pub(crate) use editor_host::EditorHost;
pub(crate) use language::{language_for_path, line_comment_prefix};
pub(crate) use metadata::{detect_indent_style, detect_line_ending, IndentStyle, LineEndingKind};
pub(crate) use session::EditorSession;

pub(crate) const APP_NAME: &str = "CyberEditor";
pub(crate) const EDITOR_CONTEXT: &str = "CyberEditor";

actions!(
    cybereditor,
    [
        OpenFile,
        SaveFile,
        SaveFileAs,
        GoToLine,
        FindText,
        ReplaceText,
        ReplaceAllText,
        ToggleComment,
        IndentSelection,
        OutdentSelection,
        FindNext,
        FindPrevious
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-o", OpenFile, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-s", SaveFile, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-s", SaveFileAs, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-g", GoToLine, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-f", FindText, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-h", ReplaceText, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-h", ReplaceAllText, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-/", ToggleComment, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-]", IndentSelection, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-[", OutdentSelection, Some(EDITOR_CONTEXT)),
        KeyBinding::new("f3", FindNext, Some(EDITOR_CONTEXT)),
        KeyBinding::new("shift-f3", FindPrevious, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-o", OpenFile, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-s", SaveFile, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-s", SaveFileAs, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-g", GoToLine, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-f", FindText, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-h", ReplaceText, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-h", ReplaceAllText, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-/", ToggleComment, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-]", IndentSelection, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-[", OutdentSelection, Some(EDITOR_CONTEXT)),
        KeyBinding::new("f3", FindNext, Some(EDITOR_CONTEXT)),
        KeyBinding::new("shift-f3", FindPrevious, Some(EDITOR_CONTEXT)),
    ]);
}
