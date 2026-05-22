use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::core::{Interface, PCSTR, PCWSTR};
use windows::Win32::Foundation::{BOOL, HANDLE, HWND, POINT};
use crate::com::ensure_com_apartment;
use windows::Win32::UI::Shell::{
    CMINVOKECOMMANDINFO, CMF_EXTENDEDVERBS, CMF_NORMAL, IContextMenu, IContextMenu3,
    ILFree, IShellFolder, SHBindToParent, SHCreateDefaultContextMenu, DEFCONTEXTMENU,
    SHParseDisplayName,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::UI::WindowsAndMessaging::{
    CopyImage, CreatePopupMenu, DestroyMenu, GetCursorPos, GetForegroundWindow, GetMenuItemCount,
    GetMenuItemInfoW, GetSubMenu, SetForegroundWindow, TrackPopupMenu, HMENU, IMAGE_BITMAP,
    LR_COPYRETURNORG, MENUITEMINFOW, MFT_SEPARATOR, MFT_STRING, MF_POPUP, MIIM_BITMAP, MIIM_FTYPE,
    MIIM_ID, MIIM_STRING, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
};

use crate::shell_icon::bitmap_to_png;

const CMD_FIRST: u32 = 1;
const CMD_LAST: u32 = 0x7fff;
const MAX_SHELL_MENU_ITEMS: usize = 96;
const MENU_ICON_PX: i32 = 16;

/// One row in a Files-style merged context flyout (not a native `TrackPopupMenu` surface).
#[derive(Debug, Clone)]
pub enum ShellContextMenuEntry {
    Separator,
    Item {
        label: String,
        command_offset: u32,
        command_string: Option<String>,
        /// PNG bytes (16×16) from the Shell menu bitmap, when present.
        icon_png: Option<Vec<u8>>,
    },
    /// Nested Shell popup (merged into flyout as submenu or flattened).
    Submenu {
        label: String,
        children: Vec<ShellContextMenuEntry>,
        icon_png: Option<Vec<u8>>,
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
        fMask: MIIM_FTYPE | MIIM_ID | MIIM_STRING | MIIM_BITMAP,
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
            let label_len = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
            let label = String::from_utf16_lossy(&label_buf[..label_len]);
            let submenu = GetSubMenu(popup, index as i32);
            if !submenu.is_invalid() {
                if let Ok(children) = enumerate_popup_menu(submenu) {
                    if !children.is_empty() {
                        entries.push(ShellContextMenuEntry::Submenu {
                            label,
                            children,
                            icon_png: menu_item_icon_png(info.hbmpItem),
                        });
                    }
                }
            }
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
            icon_png: menu_item_icon_png(info.hbmpItem),
        });
    }

    Ok(entries)
}

unsafe fn menu_item_icon_png(hbmp: HBITMAP) -> Option<Vec<u8>> {
    if hbmp.is_invalid() {
        return None;
    }
    let copy = CopyImage(
        HANDLE(hbmp.0),
        IMAGE_BITMAP,
        MENU_ICON_PX,
        MENU_ICON_PX,
        LR_COPYRETURNORG,
    )
    .ok()?;
    bitmap_to_png(HBITMAP(copy.0)).ok()
}

/// Opens the system «Open with» dialog for a file (same as Explorer).
pub fn show_open_with_dialog(path: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let path = path.to_string_lossy();
    let status = Command::new("rundll32.exe")
        .arg("shell32.dll,OpenAs_RunDLL")
        .arg(path.as_ref())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("OpenAs dialog exited with {status}")
    }
}

/// Opens the parent folder in a new Explorer window (Files «Open in new window» subset).
pub fn open_in_new_explorer_window(path: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let target = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    };
    let status = Command::new("explorer.exe")
        .arg(target.as_os_str())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("explorer exited with {status}")
    }
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
pub fn invoke_shell_context_menu_item(
    paths: &[PathBuf],
    command_offset: u32,
    extended_verbs: bool,
) -> anyhow::Result<()> {
    if paths.is_empty() || !same_parent(paths) {
        anyhow::bail!("invalid paths for shell menu invoke");
    }

    ensure_com_apartment()?;
    unsafe {
        let handle = create_context_menu(paths, extended_verbs)?;
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
