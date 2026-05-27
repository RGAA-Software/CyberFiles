# CyberEditor Code Editor Plan

This document defines the technical direction for turning `CyberEditor` from a text-input-based page into a real text and code editor.

## Current State

Current implementation:

- [crates/ui/src/cyber_editor.rs](/D:/source/CyberFiles/crates/ui/src/cyber_editor.rs)

Current capabilities:

- open file
- save / save as
- dirty state
- basic line numbers
- basic soft wrap
- syntax highlighter selection by extension
- plain single-document editing

Current limitation:

- the editor is still built on `gpui_component::input::InputState`
- this is acceptable for a lightweight text editor
- this is not the right long-term base for a code editor with diagnostics, completions, symbol navigation, diff gutter, LSP, and multi-buffer workflows

## Goal

Build `CyberEditor` into a real code editor with:

- robust text editing
- large-file-safe buffer model
- syntax-aware rendering
- project file editing
- search / replace / goto line
- diagnostics and code navigation
- staged LSP integration

Non-goal for the first phases:

- do not try to replicate all of Zed
- do not start with collaboration, agent features, terminal integration, or remote editing
- do not keep stacking advanced code-editor features directly onto `InputState`

## Architecture Decision

## Decision

`CyberEditor` should move from `InputState`-as-editor to a layered editor architecture modeled after Zed:

```text
document/buffer model
  -> display/edit model
  -> editor view
  -> project/language services
```

## Why

For a code editor, the hard part is not the toolbar or title bar. The hard part is:

- text buffer state
- selections and cursors
- display mapping
- diagnostics
- syntax state
- incremental edits
- file reload and save coordination
- future LSP integration

Those need a dedicated editor model.

## What To Reuse From Zed

Relevant Zed crates and ideas:

- `language::Buffer`
  - text buffer
  - file binding
  - syntax state
  - diagnostics storage
  - language attachment
- `multi_buffer::MultiBuffer`
  - single-file editing can still use `singleton`
  - keeps the door open for excerpts, compare views, split editing, diff views
- `editor::Editor`
  - actual editor view layer
  - selections
  - scrolling
  - gutter
  - diagnostics
  - search highlighting
  - wrapping / folding / inlays / future code actions

Minimal Zed composition pattern:

```text
Buffer
  -> MultiBuffer::singleton
  -> Editor::new / Editor::for_buffer
```

Useful example inside `../zed`:

- `../zed/crates/inspector_ui/src/div_inspector.rs`

This is the closest thing to a minimal embedded code editor.

## What Not To Import Initially

Do not pull these into the first implementation phase:

- `workspace`
- collaboration
- agent UI
- terminal integration
- git blame
- code lens
- edit prediction
- remote project plumbing

These are product-level systems, not the editor core.

## Recommended Target Shape In CyberFiles

Add dedicated editor-side modules instead of keeping everything in one page file.

Suggested near-term layout:

```text
crates/ui/src/cyber_editor/
  mod.rs
  page.rs
  core.rs
  file_io.rs
  toolbar.rs
  status_bar.rs
  search.rs
  commands.rs
  language.rs
  settings.rs
```

If the Zed-based path is adopted, add new workspace crates later:

```text
crates/editor-core
  editor-facing buffer/session abstractions for CyberFiles

crates/editor-languages
  language registry, file-extension mapping, syntax setup

crates/editor-lsp
  diagnostics, hover, completion, goto-definition integration
```

These do not need to exist at phase 1. They are the likely direction after the UI proof-of-concept is stable.

## Delivery Strategy

Build the editor in phases. Each phase should leave the app working.

## Phase 0: Planning And Spike

Goal:

- prove the editor foundation path before replacing the current page

Work:

- create a small spike branch in-tree or behind a temporary page
- construct a minimal Zed-style editor:
  - local buffer
  - multibuffer singleton
  - editor view
- load one local file into it
- verify focus, typing, selection, scrolling, and save still work

Acceptance:

- a local text file can be opened and edited
- the editor widget is not built on `InputState`
- the spike proves the stack is viable in the current workspace

## Phase 1: Editor Foundation Replacement

Goal:

- replace the `InputState` editor core while keeping the product surface simple

Work:

- keep existing `CyberEditorPage` shell
- replace internal editing widget with a proper editor model
- preserve:
  - open file
  - save
  - save as
  - dirty state
  - close confirmation
- add:
  - current line / column in status bar
  - file encoding display
  - line ending display
  - indentation mode display

Acceptance:

- existing basic workflows still work
- editor feels stable for normal text editing
- no regressions in save/open/dirty tracking

## Phase 2: Text Editor Completeness

Goal:

- make it a solid text editor before chasing IDE features

Work:

- undo / redo verification
- select all / copy / cut / paste consistency
- goto line
- find
- replace
- external file modified detection
- reload from disk
- read-only handling
- file too large fallback behavior
- drag-drop open file
- recent files

Acceptance:

- text editing workflows are reliable without code intelligence
- large files and reload conflicts have defined behavior

## Phase 3: Code Editor Essentials

Goal:

- make the editor code-oriented even before LSP

Work:

- syntax highlighting via language attachment
- bracket matching
- current line highlight
- auto indent
- tab/space settings
- comment/uncomment
- fold regions
- line numbers and gutter polish
- file-type-aware defaults
- better language detection

Acceptance:

- editing code feels materially better than editing plain text
- common source files open with the correct language mode

## Phase 4: Search And Navigation

Goal:

- support navigation workflows expected in a code editor

Work:

- in-file search panel
- replace all / replace next
- goto line and column
- symbol outline
- current file symbol navigation
- project file quick open

Acceptance:

- users can navigate medium-sized files efficiently
- project-open and file-search workflows do not require the file browser for every action

## Phase 5: Project-Aware File Editing

Goal:

- move from isolated file editing to project-aware editing

Work:

- define an editor session model separate from the file browser
- support:
  - open multiple files
  - tabs
  - split panes
  - reopen closed files
  - dirty indicators per tab
- connect editor open/save/reload to project/file-system events

Acceptance:

- the editor can be used as a multi-file code workspace
- file browser and editor stay in sync

## Phase 6: Diagnostics And LSP Foundation

Goal:

- establish the minimum language intelligence layer

Work:

- create a language service bridge
- add diagnostics model and gutter/inline rendering
- support hover
- support goto definition
- support references
- support rename symbol
- support formatting

Acceptance:

- at least one language has working diagnostics and navigation
- editor UI can show and refresh diagnostics without architectural rewrites

## Phase 7: Completion And Editing Intelligence

Goal:

- add modern code-editor interaction features

Work:

- completion popup
- snippet insertion
- signature help
- code actions
- inlay hints
- semantic tokens if needed beyond tree-sitter highlighting

Acceptance:

- typing assistance is usable
- completion and diagnostics do not freeze the UI

## Phase 8: Advanced Editor Productivity

Goal:

- add the high-value productivity features after the foundation is stable

Work:

- multi-cursor
- rectangular selection
- minimap if desired
- sticky headers if desired
- diff gutter
- inline change indicators
- compare view / read-only diff view

Acceptance:

- advanced features build on the existing architecture without rework

## Immediate Next Steps

This is the recommended implementation order for actual work:

1. create a small Zed-style editor spike
2. decide whether to vendor/adapt Zed editor crates directly or port selected concepts into local crates
3. replace `InputState` editor core
4. stabilize text-editor workflows
5. then add code-editor and project-aware features

## Two Possible Implementation Paths

## Path A: Directly depend on Zed editor stack

Meaning:

- reuse Zed crates directly where practical

Pros:

- fastest route to a real editor core
- proven architecture
- easier path to diagnostics / multibuffer / editor rendering

Cons:

- dependency surface is large
- integration complexity is real
- may pull in more project/language assumptions than CyberFiles wants

Best when:

- you want to move quickly toward a serious code editor
- you accept tighter coupling to the Zed ecosystem

## Path B: Keep current UI shell, port only the architecture and selected components

Meaning:

- copy the layered design and only adopt pieces you need

Pros:

- tighter control
- smaller long-term dependency surface
- easier to tailor to CyberFiles

Cons:

- slower
- more engineering effort
- higher risk of rebuilding hard editor problems from scratch

Best when:

- you want a product-specific editor and are willing to invest more

## Recommended Path

Recommended starting path:

- begin with a Phase 0 spike using Zed’s editor layering
- do not commit to full direct adoption until the spike proves the dependency and integration cost is acceptable
- in practice this means:
  - borrow the architecture immediately
  - test direct reuse early
  - decide full reuse vs selective local port after the spike

## Risks

- keeping `InputState` too long will create migration waste
- adopting too much of Zed too early may drag in unnecessary product complexity
- LSP should not begin before the buffer/editor foundation is stable
- multi-tab and split support should not be hacked in before editor session state is defined

## Acceptance Criteria For The Refactor Direction

The direction is correct if:

- the editor core is no longer just a text input widget
- file state, edit state, and display state are clearly separated
- adding diagnostics and goto-definition later does not require replacing the core again
- file browser integration remains explicit and understandable

## Work Resumption Checklist

When implementation begins:

1. inspect current `cyber_editor.rs`
2. create a spike page or temporary editor module
3. test `Buffer -> MultiBuffer -> Editor`
4. validate file open, edit, save, scroll, selection
5. document the dependency decision before phase 1 replacement
