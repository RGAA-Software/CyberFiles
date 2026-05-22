use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::{w, PCWSTR};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SW_SHOW};

/// Shows the system properties dialog for a file or folder.
pub fn open_item_properties(path: &Path) -> anyhow::Result<()> {
    let path_wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_INVOKEIDLIST,
        hwnd: unsafe { GetForegroundWindow() },
        lpVerb: w!("properties"),
        lpFile: PCWSTR(path_wide.as_ptr()),
        nShow: SW_SHOW.0,
        ..Default::default()
    };

    unsafe {
        ShellExecuteExW(&mut info)?;
    }
    Ok(())
}

pub use crate::context_menu::{
    invoke_shell_context_menu_item, open_in_new_explorer_window, query_shell_context_menu_items,
    show_open_with_dialog, ShellContextMenuEntry,
};

/// Optional Explorer-style popup at the cursor (not the default Files parity UX).
pub fn show_shell_context_menu(paths: &[std::path::PathBuf]) -> anyhow::Result<()> {
    crate::context_menu::show_shell_context_menu(paths)
        .or_else(|_| crate::context_menu::show_shell_context_menu_fallback(paths))
}
