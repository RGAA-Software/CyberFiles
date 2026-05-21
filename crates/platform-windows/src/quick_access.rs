//! Shell Frequent-places folder (`FOLDERID_Frequent`) — Explorer Quick Access pins.
//!
//! Files uses `3936e9e4-d92c-4eee-a85a-bc16d5ea0819` in `HomeFolder.GetQuickAccessFolderAsync`,
//! not the virtual Quick Access view (`679f85cb-…`).

use std::path::PathBuf;

use windows::core::GUID;
use windows::Win32::Foundation::{HWND, S_OK};
use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
use windows::Win32::UI::Shell::{
    ILFree, IEnumIDList, IShellFolder, SHCONTF_FOLDERS, SHCONTF_INCLUDEHIDDEN, SHGetDesktopFolder,
    SHGetKnownFolderIDList, StrRetToStrW, SHGDN_FORPARSING, SHGDN_INFOLDER, SHGDNF, KF_FLAG_DEFAULT,
};

use crate::com::ensure_com_apartment;

/// `{3936e9e4-d92c-4eee-a85a-bc16d5ea0819}` — Frequent places (pinned Quick Access folders).
const FOLDERID_FREQUENT: GUID = GUID::from_u128(0x3936e9e4_d92c_4eee_a85a_bc16d5ea0819);

const SFGAO_FOLDER: u32 = 0x2000_0000;

/// One pinned folder from the Shell Quick Access / Frequent list.
#[derive(Debug, Clone)]
pub struct ShellQuickAccessEntry {
    pub display_name: String,
    pub path: PathBuf,
}

/// Lists folders pinned to Windows Quick Access (Frequent places known folder).
pub fn list_shell_quick_access_folders() -> anyhow::Result<Vec<ShellQuickAccessEntry>> {
    ensure_com_apartment()?;
    unsafe { list_shell_quick_access_folders_inner() }
}

unsafe fn display_name_of(
    folder: &IShellFolder,
    pidl: *const ITEMIDLIST,
    flags: SHGDNF,
) -> anyhow::Result<String> {
    let mut strret = STRRET::default();
    folder.GetDisplayNameOf(pidl, flags, &mut strret)?;
    let mut psz: windows::core::PWSTR = windows::core::PWSTR::null();
    StrRetToStrW(&mut strret, Some(pidl), &mut psz)?;
    let name = psz.to_string()?;
    windows::Win32::System::Com::CoTaskMemFree(Some(psz.0 as *mut _));
    Ok(name)
}

unsafe fn is_folder_item(folder: &IShellFolder, pidl: *const ITEMIDLIST) -> bool {
    let apidl = [pidl];
    let mut attrs = 0u32;
    folder.GetAttributesOf(&apidl, &mut attrs).is_ok() && attrs & SFGAO_FOLDER != 0
}

unsafe fn enum_folder_pidls(folder: &IShellFolder) -> anyhow::Result<Vec<*mut ITEMIDLIST>> {
    let mut enum_id: Option<IEnumIDList> = None;
    let flags = (SHCONTF_FOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0) as u32;
    let hr = folder.EnumObjects(HWND::default(), flags, &mut enum_id);
    if hr != S_OK {
        anyhow::bail!("EnumObjects failed: {hr:?}");
    }
    let Some(enum_id) = enum_id else {
        return Ok(Vec::new());
    };

    let mut pidls = Vec::new();
    loop {
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        let mut fetched = 0u32;
        let mut batch = [pidl];
        let hr = enum_id.Next(&mut batch, Some(&mut fetched));
        if hr != S_OK || fetched == 0 {
            break;
        }
        pidl = batch[0];
        if pidl.is_null() {
            break;
        }
        pidls.push(pidl);
    }
    Ok(pidls)
}

unsafe fn list_shell_quick_access_folders_inner() -> anyhow::Result<Vec<ShellQuickAccessEntry>> {
    let desktop: IShellFolder = SHGetDesktopFolder()?;
    let frequent_pidl = SHGetKnownFolderIDList(&FOLDERID_FREQUENT, KF_FLAG_DEFAULT.0 as u32, None)?;
    let frequent_folder: IShellFolder = desktop.BindToObject(frequent_pidl, None)?;

    let mut entries = Vec::new();
    for pidl in enum_folder_pidls(&frequent_folder)? {
        if !is_folder_item(&frequent_folder, pidl) {
            ILFree(Some(pidl));
            continue;
        }
        let display_name =
            display_name_of(&frequent_folder, pidl, SHGDN_INFOLDER).unwrap_or_default();
        let parsing = display_name_of(&frequent_folder, pidl, SHGDN_FORPARSING).unwrap_or_default();
        let path = PathBuf::from(&parsing);
        if path.is_absolute() || parsing.starts_with(r"\\") {
            entries.push(ShellQuickAccessEntry { display_name, path });
        }
        ILFree(Some(pidl));
    }

    ILFree(Some(frequent_pidl));
    Ok(entries)
}
