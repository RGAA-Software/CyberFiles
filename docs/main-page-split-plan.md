# MainPage Split Plan

This document records the next refactor target after the `FileBrowser` module split.

## Current State

- Target file: [crates/ui/src/main_page/mod.rs](/D:/source/CyberFiles/crates/ui/src/main_page/mod.rs)
- Current size: about `1645` lines
- Main problem: state management, session restore, tab/pane actions, omnibar behavior, sidebar cache, and page rendering all live in one file
- Refactor goal: make `mod.rs` the aggregation point, not the implementation dumping ground

## Refactor Principles

- Keep behavior unchanged while moving code
- Split by responsibility, not by arbitrary line count
- Prefer modules that reduce incremental compile blast radius in high-churn areas
- Preserve public API shape where possible so surrounding code does not need broad rewrites
- Run `cargo check -p cyberfiles-ui` after each step

## Target Module Shape

Planned layout:

```text
crates/ui/src/main_page/
  mod.rs
  core.rs
  navigation.rs
  session.rs
  omnibar.rs
  sidebar.rs
  render.rs
  render_shell.rs
  tabs.rs
  info.rs
  helpers.rs
```

Not every file has to appear immediately. The split should happen in phases.

## Recommended Split Order

### Phase 1: Session and tab lifecycle

Extract the lowest-risk state logic first.

Move out:

- `encode_session_target`
- `decode_session_target`
- `capture_tab_session`
- `record_closed_tab`
- `reopen_closed_tab`
- `reopen_closed_tab_at`
- `capture_shell_layout`
- `persist_session`
- `add_tab`
- `close_tab`
- `tab_title`

Suggested files:

- `session.rs`
- `tabs.rs`

Why first:

- These methods are cohesive
- They touch rendering less than omnibar/sidebar code
- They are easy to validate with compile checks and light runtime checks

Acceptance:

- session restore still works
- reopen closed tab still works
- tab title and tab close behavior stay unchanged

### Phase 2: Navigation and active-pane helpers

Move out:

- `active_shell`
- `active_pane`
- `active_file_browser`
- `file_navigation_active`
- `navigate_to`
- `active_navigation_target`
- `open_path_in_new_tab`
- `open_path_in_secondary_pane`
- `drop_paths_on_directory`
- `toggle_dual_pane`

Suggested file:

- `navigation.rs`

Why second:

- This becomes the core command surface for `MainPage`
- It reduces coupling between top-bar UI and pane routing

Acceptance:

- navigation between Home / Path / Recycle Bin / Tag targets still works
- dual-pane behavior is unchanged
- drag-drop open/move behavior still routes to the correct pane

### Phase 3: Omnibar and search behavior

Move out:

- `ensure_omnibar_breadcrumb_callbacks`
- `omnibar_working_directory`
- `schedule_breadcrumb_drag_preview`
- `ensure_search_input`
- `focus_search_input`
- `apply_search_from_input`
- `omnibar_path_edit_active`
- `dismiss_omnibar_path_edit`
- `ensure_omnibar_path_input`
- `enter_omnibar_path_edit`
- `submit_omnibar_path`
- `resolve_path_submit`
- `omnibar_full_path_text`
- `omnibar_breadcrumbs`
- `render_omnibar`

Suggested file:

- `omnibar.rs`

Why third:

- This area changes often
- It mixes input state, breadcrumb behavior, and rendering
- Pulling it out removes a large chunk of GPUI callback code from `mod.rs`

Acceptance:

- omnibar edit mode still works
- breadcrumb drag hover open still works
- search input still updates the active file browser

### Phase 4: Sidebar and pin management

Move out:

- `refresh_sidebar_cache`
- `reload_file_tag_browsers`
- `ensure_sidebar_cache`
- `toggle_sidebar_collapsed`
- `refresh_home_widgets`
- `pin_folder_path`
- `unpin_folder_path`
- `move_pinned_folder`
- `pin_current_folder`

Suggested file:

- `sidebar.rs`

Why fourth:

- Sidebar code is self-contained but touches config and refresh flows
- It is moderately risky, but less tangled after navigation/session code is already separated

Acceptance:

- sidebar sections still load and refresh
- pin/unpin/reorder still persists
- file-tag reload behavior still updates open panes

### Phase 5: Info pane and selection-derived page state

Move out:

- `toggle_info_pane`
- `info_selection`

Suggested file:

- `info.rs`

Why fifth:

- Small module
- Easy cleanup phase after larger state modules are extracted

Acceptance:

- info pane toggle still syncs to file browsers
- current selection info still renders correctly

### Phase 6: Main render tree split

Move out:

- `render_content_column`
- `render_shelf_pane`
- `render_shell_layout_row`
- `render_tab_bar`
- `render_title_bar`
- `render_navigation_toolbar`
- `render_status_bar`
- page-level `Render for MainPage`

Suggested files:

- `render.rs`
- `render_shell.rs`

Why last:

- Render code depends on most of the previously extracted helpers
- Once state methods are already split, rendering becomes much easier to isolate cleanly

Acceptance:

- title bar, toolbar, shell layout, status center, and status bar still render the same
- no command wiring regressions

## Expected End State

After the split:

- `main_page/mod.rs` should mainly contain:
  - imports
  - constants
  - `TabEntry`
  - `MainPage` state struct
  - constructor helpers
  - module declarations
- heavy GPUI callback code should live in dedicated modules
- feature work in omnibar/sidebar/tab logic should no longer force edits in the same giant file

## Execution Notes

- Prefer moving complete method groups, not alternating individual methods
- If a helper is shared by sibling modules, make it `pub(super)` instead of keeping duplicate logic
- Only split into a new file when the boundary is stable enough to stay
- If a method group causes too many visibility leaks, stop and create a narrower boundary first

## First Step To Resume Later

When work resumes, start with Phase 1:

1. create `crates/ui/src/main_page/session.rs`
2. create `crates/ui/src/main_page/tabs.rs`
3. move session/tab lifecycle methods
4. run `cargo check -p cyberfiles-ui`
5. commit before touching omnibar or render code
