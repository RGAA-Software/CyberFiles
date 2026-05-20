use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
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

/// Invokes the default shell context menu at the cursor for the given paths.
pub fn show_shell_context_menu(paths: &[std::path::PathBuf]) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    // Full `IContextMenu` hosting requires a parent HWND and message pump integration.
    // For now, open the parent folder with the first item selected in Explorer.
    let primary = &paths[0];
    let parent = primary
        .parent()
        .filter(|p| p.exists())
        .unwrap_or(primary.as_path());
    let parent_wide: Vec<u16> = OsStr::new(parent)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let args = if paths.len() == 1 {
        format!(
            "/select,\"{}\"",
            primary.display().to_string().replace('"', "")
        )
    } else {
        String::new()
    };
    let args_wide: Vec<u16> = OsStr::new(&args)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        hwnd: HWND::default(),
        lpVerb: w!("open"),
        lpFile: PCWSTR(parent_wide.as_ptr()),
        lpParameters: if args.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(args_wide.as_ptr())
        },
        nShow: SW_SHOW.0,
        ..Default::default()
    };

    unsafe {
        ShellExecuteExW(&mut info)?;
    }
    Ok(())
}
