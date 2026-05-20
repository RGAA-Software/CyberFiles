use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::core::{Interface, PCSTR, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, POINT};
use crate::com::ensure_com_apartment;
use windows::Win32::UI::Shell::{
    CMINVOKECOMMANDINFO, CMF_EXTENDEDVERBS, CMF_NORMAL, IContextMenu, IContextMenu3,
    ILFree, IShellFolder, SHBindToParent, SHCreateDefaultContextMenu, DEFCONTEXTMENU,
    SHParseDisplayName,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, DestroyMenu, GetCursorPos, GetForegroundWindow, GetMenuItemCount,
    GetMenuItemInfoW, SetForegroundWindow, TrackPopupMenu, HMENU, MENUITEMINFOW, MFT_SEPARATOR,
    MFT_STRING, MF_POPUP, MIIM_FTYPE, MIIM_ID, MIIM_STRING, TPM_LEFTALIGN, TPM_RETURNCMD,
    TPM_RIGHTBUTTON,
};

const CMD_FIRST: u32 = 1;
const CMD_LAST: u32 = 0x7fff;
const MAX_SHELL_MENU_ITEMS: usize = 32;

/// One row in a Files-style merged context flyout (not a native `TrackPopupMenu` surface).
#[derive(Debug, Clone)]
pub enum ShellContextMenuEntry {
    Separator,
    Item {
        label: String,
        command_offset: u32,
        command_string: Option<String>,
    },
}

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

fn should_skip_shell_verb(command_string: Option<&str>, label: &str) -> bool {
    const KNOWN: &[&str] = &[
        "open",
        "opennew",
        "opencontaining",
        "opennewprocess",
        "runas",
        "runasuser",
        "cut",
        "copy",
        "paste",
        "delete",
        "properties",
        "link",
        "rename",
        "explore",
        "openinfiles",
        "extract",
        "copyaspath",
        "undelete",
        "empty",
        "format",
    ];
    if let Some(verb) = command_string {
        if KNOWN.iter().any(|k| verb.eq_ignore_ascii_case(k)) {
            return true;
        }
    }
    let lower = label.to_ascii_lowercase();
    KNOWN.iter().any(|k| lower == *k)
}

struct ContextMenuHandle {
    menu: IContextMenu,
    popup: HMENU,
    child_pidls: Vec<*mut ITEMIDLIST>,
}

impl ContextMenuHandle {
    unsafe fn release(self) {
        let _ = DestroyMenu(self.popup);
        for pidl in self.child_pidls {
            free_pidl(pidl);
        }
    }
}

unsafe fn create_context_menu(paths: &[PathBuf], extended_verbs: bool) -> anyhow::Result<ContextMenuHandle> {
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
    let flags = if extended_verbs {
        CMF_NORMAL | CMF_EXTENDEDVERBS
    } else {
        CMF_NORMAL
    };
    menu.QueryContextMenu(popup, 0, CMD_FIRST, CMD_LAST, flags)?;

    Ok(ContextMenuHandle {
        menu,
        popup,
        child_pidls,
    })
}

unsafe fn enumerate_popup_menu(popup: HMENU) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    let count = GetMenuItemCount(popup);
    if count == 0 {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut info = MENUITEMINFOW {
        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_FTYPE | MIIM_ID | MIIM_STRING,
        ..Default::default()
    };

    for index in 0..count as u32 {
        if entries.len() >= MAX_SHELL_MENU_ITEMS {
            break;
        }

        let mut label_buf = [0u16; 512];
        info.dwTypeData = windows::core::PWSTR(label_buf.as_mut_ptr());
        info.cch = label_buf.len() as u32;

        if GetMenuItemInfoW(popup, index, true, &mut info).is_err() {
            continue;
        }

        if info.fType.0 & MFT_SEPARATOR.0 != 0 {
            entries.push(ShellContextMenuEntry::Separator);
            continue;
        }

        if info.fType.0 & MF_POPUP.0 != 0 {
            continue;
        }

        if info.fType.0 & MFT_STRING.0 == 0 {
            continue;
        }

        let label_len = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
        let label = String::from_utf16_lossy(&label_buf[..label_len]);
        let command_offset = info.wID.saturating_sub(CMD_FIRST);

        if should_skip_shell_verb(None, &label) {
            continue;
        }

        entries.push(ShellContextMenuEntry::Item {
            label,
            command_offset,
            command_string: None,
        });
    }

    Ok(entries)
}

/// Enumerates Shell context menu entries for merging into a GPUI flyout (Files-style).
pub fn query_shell_context_menu_items(
    paths: &[PathBuf],
    extended_verbs: bool,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    if paths.is_empty() || !same_parent(paths) {
        return Ok(Vec::new());
    }

    ensure_com_apartment()?;
    unsafe {
        let handle = create_context_menu(paths, extended_verbs)?;
        let entries = enumerate_popup_menu(handle.popup)?;
        handle.release();
        Ok(entries)
    }
}

/// Invokes one Shell menu command by offset (from [`query_shell_context_menu_items`]).
pub fn invoke_shell_context_menu_item(paths: &[PathBuf], command_offset: u32) -> anyhow::Result<()> {
    if paths.is_empty() || !same_parent(paths) {
        anyhow::bail!("invalid paths for shell menu invoke");
    }

    ensure_com_apartment()?;
    unsafe {
        let handle = create_context_menu(paths, false)?;
        let hwnd = GetForegroundWindow();
        let mut info = CMINVOKECOMMANDINFO::default();
        info.cbSize = std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32;
        info.hwnd = hwnd;
        info.lpVerb = PCSTR::from_raw(command_offset as usize as *const u8);
        info.nShow = 1;
        handle.menu.InvokeCommand(&info)?;
        handle.release();
        Ok(())
    }
}

/// Optional Explorer-style popup (not the default Files parity UX).
pub fn show_shell_context_menu(paths: &[PathBuf]) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    if !same_parent(paths) {
        return show_shell_context_menu_fallback(paths);
    }

    ensure_com_apartment()?;
    unsafe { show_shell_context_menu_inner(paths) }
}

unsafe fn show_shell_context_menu_inner(paths: &[PathBuf]) -> anyhow::Result<()> {
    let handle = create_context_menu(paths, false)?;
    let hwnd = GetForegroundWindow();

    let mut cursor = POINT::default();
    GetCursorPos(&mut cursor)?;
    let _ = SetForegroundWindow(hwnd);

    let cmd = TrackPopupMenu(
        handle.popup,
        TPM_RETURNCMD | TPM_LEFTALIGN | TPM_RIGHTBUTTON,
        cursor.x,
        cursor.y,
        0,
        hwnd,
        None,
    );

    if cmd == BOOL(0) {
        handle.release();
        return Ok(());
    }

    let offset = cmd.0 as u32 - CMD_FIRST;
    let mut info = CMINVOKECOMMANDINFO::default();
    info.cbSize = std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32;
    info.hwnd = hwnd;
    info.lpVerb = PCSTR::from_raw(offset as usize as *const u8);
    info.nShow = 1;
    handle.menu.InvokeCommand(&info)?;
    handle.release();
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
