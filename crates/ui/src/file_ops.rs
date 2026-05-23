//! Background copy/move with status notifications (Files StatusCenter subset).

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

use cyberfiles_fs::{
    paths_conflict, transfer_one, ClipboardOperation, ConflictResolution, FileClipboard,
    TransferOutcome,
};
use gpui::{AppContext, Context, Entity, SharedString, WeakEntity, Window};
use gpui_component::{
    dialog::DialogButtonProps,
    notification::Notification,
    WindowExt as _,
};
use rust_i18n::t;

use crate::app_state::AppFileClipboard;
use crate::file_browser::FileBrowser;

#[derive(Clone, Copy)]
pub enum FileTransferKind {
    Copy,
    Move,
}

fn operation_for_kind(kind: FileTransferKind) -> ClipboardOperation {
    match kind {
        FileTransferKind::Copy => ClipboardOperation::Copy,
        FileTransferKind::Move => ClipboardOperation::Cut,
    }
}

/// Run copy/move off the UI thread; show in-progress and result notifications.
pub fn spawn_file_transfer(
    browser: Entity<FileBrowser>,
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
    let weak = browser.downgrade();
    cx.spawn(async move |this, cx| {
        let result = run_transfer_with_conflicts(weak, cx, kind, sources, destination).await;

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                cx.notify();
                return;
            };
            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(outcome) if outcome.cancelled => {
                    window.push_notification(
                        Notification::info(t!("files.transfer.cancelled")),
                        cx,
                    );
                }
                Ok(outcome) if outcome.transferred > 0 => {
                    window.push_notification(Notification::success(t!("files.transfer.done")), cx);
                }
                Ok(_) => {}
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!("files.transfer.failed"))),
                        cx,
                    );
                }
            });

            if matches!(result, Ok(outcome) if outcome.transferred > 0)
                && *browser.current_directory() == dest_for_reload
            {
                browser.reload();
            }
            cx.notify();
        });
    })
    .detach();
}

/// Paste from a taken clipboard (same semantics as synchronous paste, but non-blocking).
pub fn spawn_paste_from_clipboard(
    browser: Entity<FileBrowser>,
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

    let weak = browser.downgrade();
    cx.spawn(async move |this, cx| {
        let result = run_transfer_with_conflicts(weak, cx, kind, paths, destination).await;

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                cx.notify();
                return;
            };
            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(outcome) if outcome.cancelled => {
                    AppFileClipboard::set(
                        FileClipboard::new(operation, paths_for_clipboard.clone()),
                        cx,
                    );
                    window.push_notification(
                        Notification::info(t!("files.transfer.cancelled")),
                        cx,
                    );
                }
                Ok(outcome) if outcome.transferred > 0 => {
                    if operation == ClipboardOperation::Copy {
                        AppFileClipboard::store(operation, paths_for_clipboard.clone(), cx);
                    }
                    window.push_notification(Notification::success(t!("files.paste.success")), cx);
                }
                Ok(_) => {
                    AppFileClipboard::set(
                        FileClipboard::new(operation, paths_for_clipboard.clone()),
                        cx,
                    );
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

            if matches!(result, Ok(outcome) if outcome.transferred > 0) {
                browser.reload();
            }
            cx.notify();
        });
    })
    .detach();
}

async fn run_transfer_with_conflicts(
    _browser: WeakEntity<FileBrowser>,
    cx: &mut gpui::AsyncApp,
    kind: FileTransferKind,
    sources: Vec<PathBuf>,
    destination: PathBuf,
) -> anyhow::Result<TransferOutcome> {
    let operation = operation_for_kind(kind);
    let mut skip_all = false;
    let mut replace_all = false;
    let mut outcome = TransferOutcome::default();

    for source in sources {
        let file_name = source
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid source path {}", source.display()))?;
        let target = destination.join(file_name);

        if paths_conflict(&source, &target) {
            if skip_all {
                continue;
            }
            if !replace_all {
                let resolution = prompt_conflict(cx, &source, &target).await;
                match resolution {
                    ConflictResolution::Skip => continue,
                    ConflictResolution::SkipAll => {
                        skip_all = true;
                        continue;
                    }
                    ConflictResolution::Replace => {}
                    ConflictResolution::ReplaceAll => replace_all = true,
                    ConflictResolution::Cancel => {
                        outcome.cancelled = true;
                        return Ok(outcome);
                    }
                }
            }
        }

        let must_replace = paths_conflict(&source, &target);
        let source_path = source.clone();
        let dest_dir = destination.clone();
        cx.background_spawn(async move {
            transfer_one(&source_path, &dest_dir, operation, must_replace)
        })
        .await?;
        outcome.transferred += 1;
    }

    Ok(outcome)
}

async fn prompt_conflict(
    cx: &mut gpui::AsyncApp,
    source: &Path,
    target: &Path,
) -> ConflictResolution {
    let (tx, rx) = mpsc::sync_channel(1);
    let tx = Arc::new(tx);
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| source.display().to_string());
    let target_display = target.display().to_string();
    let title = SharedString::from(t!("files.conflict.title"));
    let description = SharedString::from(t!(
        "files.conflict.description",
        name = name,
        path = target_display
    ));
    let replace_label = SharedString::from(t!("files.conflict.replace"));
    let skip_label = SharedString::from(t!("files.conflict.skip"));

    let _ = cx.update(|cx| {
        let Some(window) = cx.active_window() else {
            let _ = tx.send(ConflictResolution::Cancel);
            return;
        };
        let _ = window.update(cx, |_, window, cx| {
            let tx_ok = tx.clone();
            let tx_skip = tx.clone();
            window.open_alert_dialog(cx, move |alert, _window, _cx| {
                alert
                    .title(title.clone())
                    .description(description.clone())
                    .button_props(
                        DialogButtonProps::default()
                            .ok_text(replace_label.clone())
                            .cancel_text(skip_label.clone())
                            .show_cancel(true),
                    )
                    .on_ok({
                        let tx_ok = tx_ok.clone();
                        move |_, _, _| {
                            let _ = tx_ok.send(ConflictResolution::Replace);
                            true
                        }
                    })
                    .on_cancel({
                        let tx_skip = tx_skip.clone();
                        move |_, _, _| {
                            let _ = tx_skip.send(ConflictResolution::Skip);
                            true
                        }
                    })
            });
        });
    });

    cx.background_spawn(async move { rx.recv().unwrap_or(ConflictResolution::Cancel) })
        .await
}
