use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::core::{Interface, PCSTR, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, POINT};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    CMINVOKECOMMANDINFO, CMF_NORMAL, IContextMenu, IContextMenu3, ILFree, IShellFolder,
    SHBindToParent, SHCreateDefaultContextMenu, DEFCONTEXTMENU, SHParseDisplayName,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, DestroyMenu, GetCursorPos, GetForegroundWindow, SetForegroundWindow,
    TrackPopupMenu, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
};

const CMD_FIRST: u32 = 1;
const CMD_LAST: u32 = 0x7fff;

fn path_to_wide(path: &Path) -> Vec<u16> {
    OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn same_parent(paths: &[PathBuf]) -> bool {
    let Some(first) = paths.first().and_then(|p| p.parent().map(|p| p.to_path_buf())) else {
        return false;
    };
    paths
        .iter()
        .all(|p| p.parent().map(|p| p.to_path_buf()) == Some(first.clone()))
}

unsafe fn bind_parent_and_relative(path: &Path) -> anyhow::Result<(IShellFolder, *mut ITEMIDLIST)> {
    let wide = path_to_wide(path);
    let mut full_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut full_pidl, 0, None)?;

    let mut relative: *mut ITEMIDLIST = std::ptr::null_mut();
    let parent: IShellFolder = SHBindToParent(full_pidl, Some(&mut relative))?;
    ILFree(Some(full_pidl));
    Ok((parent, relative))
}

unsafe fn free_pidl(pidl: *mut ITEMIDLIST) {
    if !pidl.is_null() {
        ILFree(Some(pidl));
    }
}

/// Shows the Explorer system context menu for local paths in the same folder.
pub fn show_shell_context_menu(paths: &[PathBuf]) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    if !same_parent(paths) {
        return show_shell_context_menu_fallback(paths);
    }

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let result = show_shell_context_menu_inner(paths);
        CoUninitialize();
        result
    }
}

unsafe fn show_shell_context_menu_inner(paths: &[PathBuf]) -> anyhow::Result<()> {
    let (parent_sf, first_child) = bind_parent_and_relative(&paths[0])?;
    let mut child_pidls = vec![first_child];

    for path in paths.iter().skip(1) {
        let (_, relative) = bind_parent_and_relative(path)?;
        child_pidls.push(relative);
    }

    let hwnd = GetForegroundWindow();
    let apidl: Vec<*const ITEMIDLIST> = child_pidls
        .iter()
        .map(|p| *p as *const ITEMIDLIST)
        .collect();

    let dcm = DEFCONTEXTMENU {
        hwnd,
        pcmcb: Default::default(),
        pidlFolder: std::ptr::null_mut(),
        psf: std::mem::ManuallyDrop::new(Some(parent_sf.clone())),
        cidl: apidl.len() as u32,
        apidl: apidl.as_ptr() as *mut *mut ITEMIDLIST,
        punkAssociationInfo: Default::default(),
        cKeys: 0,
        aKeys: std::ptr::null(),
    };

    let menu: IContextMenu = SHCreateDefaultContextMenu(&dcm)?;
    let _menu3: IContextMenu3 = menu.cast()?;

    let popup = CreatePopupMenu()?;
    menu.QueryContextMenu(popup, 0, CMD_FIRST, CMD_LAST, CMF_NORMAL)?;

    let mut cursor = POINT::default();
    GetCursorPos(&mut cursor)?;
    let _ = SetForegroundWindow(hwnd);

    let cmd = TrackPopupMenu(
        popup,
        TPM_RETURNCMD | TPM_LEFTALIGN | TPM_RIGHTBUTTON,
        cursor.x,
        cursor.y,
        0,
        hwnd,
        None,
    );

    if cmd == BOOL(0) {
        DestroyMenu(popup)?;
        for pidl in child_pidls {
            free_pidl(pidl);
        }
        return Ok(());
    }

    let offset = cmd.0 as u32 - CMD_FIRST;
    let mut info = CMINVOKECOMMANDINFO::default();
    info.cbSize = std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32;
    info.hwnd = hwnd;
    info.lpVerb = PCSTR::from_raw(offset as usize as *const u8);
    info.nShow = 1;
    menu.InvokeCommand(&info)?;

    DestroyMenu(popup)?;

    for pidl in child_pidls {
        free_pidl(pidl);
    }

    Ok(())
}

/// Fallback when COM menu setup fails: open parent folder in Explorer.
pub fn show_shell_context_menu_fallback(paths: &[PathBuf]) -> anyhow::Result<()> {
    use windows::core::w;
    use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

    let primary = &paths[0];
    let parent = primary
        .parent()
        .filter(|p| p.exists())
        .unwrap_or(primary.as_path());
    let parent_wide = path_to_wide(parent);
    let args = if paths.len() == 1 {
        format!(
            "/select,\"{}\"",
            primary.display().to_string().replace('"', "")
        )
    } else {
        String::new()
    };
    let args_wide = path_to_wide(Path::new(&args));

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
