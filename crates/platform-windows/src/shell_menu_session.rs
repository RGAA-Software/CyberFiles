//! Active Shell context menu session (Files: `ContextMenu` + `ThreadWithMessageQueue`).

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::com::StaMessageThread;
use crate::context_menu::{self, ShellContextMenuEntry};

const QUERY_TIMEOUT: Duration = Duration::from_secs(10);

static ACTIVE_SESSION: OnceLock<Mutex<Option<Arc<ShellMenuSession>>>> = OnceLock::new();

fn active_session() -> &'static Mutex<Option<Arc<ShellMenuSession>>> {
    ACTIVE_SESSION.get_or_init(|| Mutex::new(None))
}

/// Owns the STA thread that created the current `IContextMenu` (invoke must use the same thread).
struct ShellMenuSession {
    thread: Arc<StaMessageThread>,
}

fn replace_session() -> Arc<ShellMenuSession> {
    if let Ok(mut guard) = active_session().lock() {
        if let Some(old) = guard.take() {
            old.thread.dispatch(context_menu::release_prepared_menu);
        }
        let session = Arc::new(ShellMenuSession {
            thread: Arc::new(StaMessageThread::new("cyberfiles-shell-menu")),
        });
        *guard = Some(session.clone());
        session
    } else {
        Arc::new(ShellMenuSession {
            thread: Arc::new(StaMessageThread::new("cyberfiles-shell-menu")),
        })
    }
}

pub fn clear_session() {
    if let Ok(mut guard) = active_session().lock() {
        if let Some(session) = guard.take() {
            session.thread.dispatch(context_menu::release_prepared_menu);
        }
    }
}

/// Query Shell verbs on a dedicated STA thread; keeps `IContextMenu` alive for [`invoke_on_session`].
pub fn query_with_session(
    paths: &[PathBuf],
    extended_verbs: bool,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    let session = replace_session();
    let paths = paths.to_vec();
    let paths_log = paths.clone();
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    session.thread.dispatch(move || {
        let result = context_menu::prepare_and_enumerate(&paths, extended_verbs);
        let _ = done_tx.send(result);
    });

    match done_rx.recv_timeout(QUERY_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            eprintln!(
                "[shell-menu] query timed out after {}s paths={paths_log:?}",
                QUERY_TIMEOUT.as_secs(),
            );
            clear_session();
            Err(anyhow::anyhow!(
                "Shell context menu query timed out after {}s",
                QUERY_TIMEOUT.as_secs()
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            clear_session();
            Err(anyhow::anyhow!("Shell context menu worker exited"))
        }
    }
}

/// Invoke a command on the session STA thread (Files: `_owningThread.PostMethod`).
pub fn invoke_on_session(command_offset: u32) -> anyhow::Result<()> {
    let session = active_session()
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .ok_or_else(|| anyhow::anyhow!("no active shell context menu session"))?;
    session
        .thread
        .post(move || context_menu::invoke_prepared_menu(command_offset))
}
