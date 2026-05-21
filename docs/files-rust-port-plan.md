# Files Rust Port Plan

This document is the working plan for **一比一复刻** the neighboring `../Files` project in Rust inside CyberFiles. The source project is a WinUI 3 file manager with deep Windows Shell integration; the port is organized by capability layers instead of direct C# to Rust translation.

## Parity principle (Path B)

- **Product reference is Files, not Explorer.** UI, commands, and workflows should match `../Files` behavior and settings semantics before matching generic Windows shell defaults.
- **Stack:** Rust + **GPUI** + **gpui-component** for all in-app UI (tabs, toolbar, layouts, context flyouts, settings). Win32/COM lives only in `platform-windows` and is consumed as services (enumerate / invoke), not as replacement UI unless Files does so explicitly.
- **UI 执行规范（硬性）：** 所有组件必须使用 gpui-component；库中无对应能力时须提示维护者，见 [`execution-guidelines.md`](execution-guidelines.md)。
- **Context menus:** Files uses in-app `CommandBarFlyout` plus parsed `IContextMenu` items — see [`files-context-menu-parity.md`](files-context-menu-parity.md). Do **not** use `TrackPopupMenu` as the default right-click experience.

## Goals

- Recreate the user-facing **Files** experience in a Rust desktop app (feature parity tracked in `files-parity-roadmap.md`).
- Keep the core file model and operations UI-toolkit independent.
- Use GPUI and `gpui-component` for the application shell and controls.
- Isolate Windows-specific Shell, COM, and Win32 behavior behind platform crates.
- Deliver usable milestones instead of waiting for every advanced Files feature.

## Non-Goals For The First Milestones

- Do not implement Windows Open/Save dialog replacement first.
- Do not default to Explorer-style native `TrackPopupMenu` context menus (Shell items are merged into GPUI menus per Files).
- Do not implement every cloud provider, FTP, archive, Git, and tag feature before the local file browser is stable.
- Do not copy the WinUI MVVM structure one-to-one. The Rust version should use explicit state, commands, and services that fit GPUI.

## Target Workspace Shape

```text
crates/app
  Binary entry point.

crates/core
  App constants, config, persisted settings, shared value types.

crates/fs
  Toolkit-independent file item model, local directory enumeration, sorting, filtering,
  and later filesystem operations.

crates/ui
  GPUI shell, sidebar, tabs, toolbar, file list views, settings, dialogs.

crates/commands
  Command registry, hotkeys, command enablement, command labels/icons.

crates/platform-windows
  Windows-only Shell paths, CF_HDROP clipboard, icon hints, properties dialog.

crates/previews
  Preview provider abstraction and built-in text/image/markdown/folder previews.

crates/search
  Folder search, incremental search jobs, future indexing.

crates/tags
  File tag database and tag metadata.

crates/archive
  Archive browsing and extraction/compression.

crates/git
  Git status and commit metadata.
```

Workspace crates today: `app`, `core`, `fs`, `commands`, `ui`, `platform-windows`. Additional crates (`previews`, `search`, `tags`, …) are added when their boundary becomes concrete.

## Source Project Capability Map

### Application Lifecycle

Files handles single-instance activation, command-line parsing, protocol activation, file activation, session restore, splash screen, background tray mode, and graceful shutdown.

Rust plan:

- Phase 1: normal app launch and persisted window size.
- Phase 2: single-instance behavior and command-line open path.
- Phase 3: protocol/file activation and background tray behavior.

### Main Window Layout

Files uses this high-level structure:

- Tab bar
- Navigation/address toolbar
- Sidebar
- Main shell content
- Optional info/preview pane
- Status bar
- Shelf pane

Rust plan:

- Phase 1: sidebar + toolbar + one content pane + status bar.
- Phase 2: multiple tabs, each with independent navigation history.
- Phase 3: dual pane and shelf pane.
- Phase 4: preview/info pane positioning right/bottom.

### File Item Model

Files centers around a `ListedItem` model that represents files, folders, shortcuts, recycle bin entries, FTP entries, ZIP entries, libraries, alternate streams, and Git-aware items.

Rust plan:

- Phase 1: local files, folders, symlinks, unknown items.
- Phase 2: shortcut and drive root metadata.
- Phase 3: recycle bin, library, archive, FTP, Git, cloud sync, tags.

### Directory Enumeration

Files has fast Windows-specific enumeration, hidden/system filtering, thumbnail loading, incremental updates, sorting/grouping, and watcher-driven refresh.

Rust plan:

- Phase 1: `std::fs::read_dir` based local enumeration.
- Phase 2: `notify` watcher and batched refresh.
- Phase 3: Windows-specialized enumeration with attributes, icons, shell names, and thumbnails.
- Phase 4: virtual folders such as Home, Recycle Bin, Libraries, archive folders, and FTP.

### Sorting And Grouping

Files sorts by name, modified date, created date, size, type, sync status, tag, original folder, deleted date, and path. It groups by similar dimensions.

Rust plan:

- Phase 1: name, modified date, created date, size, type, path.
- Phase 2: grouping by name/date/type/size.
- Phase 3: sync/tag/recycle-bin/library specific fields.

### File Operations

Files supports create, rename, copy, move, delete, recycle, restore, paste, drag/drop, archive operations, Git actions, background actions, image actions, tags, pinning, and sharing.

Rust plan:

- Phase 1: create folder/file, rename, open, reveal, copy path.
- Phase 2: copy, move, trash, permanent delete, clipboard paste.
- Phase 3: drag/drop, progress center, conflict dialogs.
- Phase 4: archive, Git, image, tags, pinning, sharing.

### Commands And Hotkeys

Files has a rich command system with command codes, labels, icons, hotkeys, variants, and context-sensitive enablement.

Rust plan:

- Phase 1: explicit enum and registry for core file commands.
- Phase 2: configurable hotkeys and toolbar/context menu generation from the registry.
- Phase 3: user-customizable commands and command palette.

### Sidebar And Home

Files sidebar includes pinned folders, libraries, drives, cloud drives, network, WSL, file tags, and settings. Home includes widgets for drives, recent files, quick access, network locations, and tags.

Rust plan:

- Phase 1: Home, common user folders, drives, settings.
- Phase 2: pinned folders, recent folders, network placeholders.
- Phase 3: libraries, cloud drives, WSL, tags.

### Preview And Info Pane

Files has previews for basic files, text, code, markdown, image, media, PDF, rich text, HTML, archive, shortcut, shell preview, and folder summary.

Rust plan:

- Phase 1: folder summary, text, markdown, image.
- Phase 2: code highlighting and common media metadata.
- Phase 3: PDF, archive, rich text, shell preview handlers.

## Milestones

### Milestone 0: Planning And Core File Model

Deliverables:

- This plan document.
- `cyberfiles-fs` crate.
- `FileItem`, `FileItemKind`, local directory read, sort preferences.
- Unit tests for sorting and display-name behavior.

Done when:

- `cargo check` passes.
- The file model can enumerate a local directory without UI dependencies.

### Milestone 1: Real Files Page

Deliverables:

- Replace the Files placeholder page with a real directory listing.
- Default path: home directory or primary drive.
- Toolbar shows current path, refresh, up, view selector.
- Status bar shows item count and selection count.

Done when:

- The app can browse local folders.
- Double-click opens folders.
- File rows show name, type, size, modified date.

### Milestone 2: Navigation And Selection

Deliverables:

- Per-tab navigation state.
- Back, forward, up, refresh.
- Multi-select with Ctrl/Shift.
- Keyboard navigation and Enter/F2/Delete shortcuts.

Done when:

- Navigation history works per tab.
- Selection survives refresh where possible.

### Milestone 3: File Operations

Deliverables:

- New folder/file.
- Rename.
- Open with system default.
- Copy path.
- Move to trash and permanent delete.
- Basic operation errors surfaced as dialogs/notifications.

Done when:

- The app is useful for everyday local browsing and simple mutations.

### Milestone 4: Watchers, Search, And View Modes

Deliverables:

- Batched directory watcher refresh.
- Search within current folder.
- Details, list, and grid layouts.
- Sort and group menus.

Done when:

- External file changes appear without manual refresh.
- Large folders remain responsive enough for daily use.

### Milestone 5: Windows Shell Depth

Deliverables:

- Windows icon provider.
- Drive metadata.
- Recycle bin integration.
- Shortcut metadata.
- Files-style context flyout: GPUI menu + `query_shell_context_menu_items` merge (see `files-context-menu-parity.md`).

Done when:

- Windows-specific behavior starts matching Files rather than a generic cross-platform file browser.

### Milestone 6: Advanced Files Features

Deliverables:

- Tags database.
- Git status/commit columns.
- Archive browsing and extraction/compression.
- FTP.
- Cloud sync status.
- Preview providers beyond text/image/markdown.

Done when:

- Feature parity is tracked item by item against `../Files`.

## Immediate Implementation Order

1. Build `cyberfiles-fs`.
2. Add a GPUI file browser state in `crates/ui`.
3. Replace the current Files placeholder with real local directory data.
4. Add navigation and selection.
5. Add basic operations.

Each step should compile independently and preserve the existing settings/theme work.

## Path B: 一比一复刻追踪

长期功能对照与状态见 **[files-parity-roadmap.md](./files-parity-roadmap.md)**。

**当前进度（2026-05）：**

- **MainPage**：TabBar、侧栏、Omnibar、双栏 `ShellPanes`、InfoPane（Details + Preview）、StatusBar。
- **M3**：新建文件、应用内复制/剪切/粘贴、回收站删除（`trash`）、Pinned 写入 `settings.json`。
- **M4（首轮）**：`notify` 目录监视、`filter_items_by_query` 搜索框、详情/网格视图切换。
- **`platform-windows` crate**：图标类型提示、CF_HDROP 粘贴、回收站 Shell 枚举、Shell 菜单查询/invoke（供 GPUI 右键合并）、属性对话框。
