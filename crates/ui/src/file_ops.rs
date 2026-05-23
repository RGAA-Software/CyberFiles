//! Background copy/move with status notifications (Files StatusCenter subset).

use std::path::{Path, PathBuf};

use cyberfiles_fs::{copy_items, move_items, ClipboardOperation, FileClipboard};
use gpui::{AppContext, Context, Entity, Window};
use gpui_component::{notification::Notification, WindowExt as _};
use rust_i18n::t;

use crate::app_state::AppFileClipboard;
use crate::file_browser::FileBrowser;

#[derive(Clone, Copy)]
pub enum FileTransferKind {
    Copy,
    Move,
}

/// Run copy/move off the UI thread; show in-progress and result notifications.
pub fn spawn_file_transfer(
    _browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
    kind: FileTransferKind,
    sources: Vec<PathBuf>,
    destination: PathBuf,
) {
    if sources.is_empty() {
        return;
    }

    let count = sources.len();
    let progress = match kind {
        FileTransferKind::Copy => t!("files.transfer.copying", count = count),
        FileTransferKind::Move => t!("files.transfer.moving", count = count),
    };
    window.push_notification(Notification::info(progress), cx);

    let dest_for_reload = destination.clone();
    cx.spawn(async move |this, cx| {
        let result = cx
            .background_spawn(async move { perform_transfer(kind, &sources, &destination) })
            .await;

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                cx.notify();
                return;
            };
            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(()) => {
                    window.push_notification(Notification::success(t!("files.transfer.done")), cx);
                }
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!("files.transfer.failed"))),
                        cx,
                    );
                }
            });

            if matches!(result, Ok(())) && *browser.current_directory() == dest_for_reload {
                browser.reload();
            }
            cx.notify();
        });
    })
    .detach();
}

/// Paste from a taken clipboard (same semantics as synchronous paste, but non-blocking).
pub fn spawn_paste_from_clipboard(
    _browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
    clipboard: FileClipboard,
    destination: PathBuf,
) {
    if clipboard.paths.is_empty() {
        return;
    }
    let kind = match clipboard.operation {
        ClipboardOperation::Copy => FileTransferKind::Copy,
        ClipboardOperation::Cut => FileTransferKind::Move,
    };
    let operation = clipboard.operation;
    let paths = clipboard.paths;
    let paths_for_clipboard = paths.clone();

    window.push_notification(
        Notification::info(t!("files.transfer.pasting", count = paths.len())),
        cx,
    );

    cx.spawn(async move |this, cx| {
        let result = cx
            .background_spawn(async move { perform_transfer(kind, &paths, &destination) })
            .await;

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                cx.notify();
                return;
            };
            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(()) => {
                    if operation == ClipboardOperation::Copy {
                        AppFileClipboard::store(operation, paths_for_clipboard.clone(), cx);
                    }
                    window.push_notification(Notification::success(t!("files.paste.success")), cx);
                }
                Err(error) => {
                    AppFileClipboard::set(
                        FileClipboard::new(operation, paths_for_clipboard.clone()),
                        cx,
                    );
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!("files.paste.error"))),
                        cx,
                    );
                }
            });

            if matches!(result, Ok(())) {
                browser.reload();
            }
            cx.notify();
        });
    })
    .detach();
}

fn perform_transfer(
    kind: FileTransferKind,
    sources: &[PathBuf],
    destination: &Path,
) -> anyhow::Result<()> {
    match kind {
        FileTransferKind::Copy => copy_items(sources, destination),
        FileTransferKind::Move => move_items(sources, destination),
    }
}
