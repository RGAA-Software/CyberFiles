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
- If compile speed is the primary KPI, prioritize high-churn UI/interaction code before low-churn restore logic
- Run `cargo check -p cyberfiles-ui` after each step

## Compile-Speed Reality Check

This split should help **incremental** compile time most when day-to-day edits stay inside one extracted module.

- It is unlikely to materially improve clean builds by itself
- It helps most in high-churn areas with heavy GPUI callback/render code
- Simply moving rarely touched methods into new files is good cleanup, but usually a weaker compile-time win
- If a split causes many `pub(super)` leaks or shared helper churn, the compile benefit drops because edits still fan out across sibling modules

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

The phases below are ordered for **lowest refactor risk**.

If the main goal is **developer compile feedback speed**, use the fast-path order in the next section instead of following this list mechanically.

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

## Compile-Speed First Order

If the team is optimizing for faster edit-compile-check loops rather than lowest migration risk, prefer this order:

1. `omnibar.rs`
2. `sidebar.rs`
3. `render.rs` and `render_shell.rs`
4. `navigation.rs`
5. `tabs.rs`
6. `session.rs`
7. `info.rs`

Why this order:

- `omnibar` and toolbar/search interactions are likely high-churn during feature parity work
- `sidebar` also changes often and currently mixes cache/loading/state update code into `mod.rs`
- `render` is the largest surface for ordinary UI iteration, so isolating it early shrinks the most common edit zone
- `tabs` and `session` are worth splitting, but they are less likely to dominate day-to-day compile churn

Suggested execution pattern:

1. First isolate the high-churn UI surfaces
2. Then isolate routing helpers that those UI surfaces call
3. Leave low-frequency persistence/session code for a later cleanup pass unless it blocks visibility boundaries

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
- After each extracted module, keep editing inside the new file for follow-up work; otherwise the compile-speed benefit is mostly theoretical

## First Step To Resume Later

When work resumes, start with Phase 1:

1. create `crates/ui/src/main_page/session.rs`
2. create `crates/ui/src/main_page/tabs.rs`
3. move session/tab lifecycle methods
4. run `cargo check -p cyberfiles-ui`
5. commit before touching omnibar or render code
