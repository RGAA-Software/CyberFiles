use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::SystemTime;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, S_OK};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    ILFree, IEnumIDList, IShellFolder, SHCONTF_FOLDERS, SHCONTF_INCLUDEHIDDEN, SHCONTF_NONFOLDERS,
    SHGDN_FORPARSING, SHGDN_INFOLDER, SHGetDesktopFolder, SHParseDisplayName,
};
use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
use windows::Win32::UI::Shell::{StrRetToStrW, SHGDNF};

/// One item in the virtual recycle bin (not a direct filesystem path).
#[derive(Debug, Clone)]
pub struct RecycleBinEntry {
    pub display_name: String,
    /// Parsing path for Shell verbs (properties, context menu).
    pub shell_path: PathBuf,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
}

const RECYCLE_BIN_NAMESPACE: &str = r"::\{645FF040-5081-101B-9F08-00AA002F954E\}";

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
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

/// Enumerates deleted items via the Shell recycle-bin namespace.
pub fn list_recycle_bin_entries() -> anyhow::Result<Vec<RecycleBinEntry>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let result = list_recycle_bin_entries_inner();
        CoUninitialize();
        result
    }
}

unsafe fn list_recycle_bin_entries_inner() -> anyhow::Result<Vec<RecycleBinEntry>> {
    let desktop: IShellFolder = SHGetDesktopFolder()?;

    let recycle_wide = wide(RECYCLE_BIN_NAMESPACE);
    let mut recycle_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    SHParseDisplayName(
        PCWSTR(recycle_wide.as_ptr()),
        None,
        &mut recycle_pidl,
        0,
        None,
    )?;

    let recycle_folder: IShellFolder = desktop.BindToObject(recycle_pidl, None)?;
    let mut enum_id: Option<IEnumIDList> = None;
    let flags = (SHCONTF_FOLDERS.0 | SHCONTF_NONFOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0) as u32;
    let hr = recycle_folder.EnumObjects(HWND::default(), flags, &mut enum_id);
    if hr != S_OK {
        ILFree(Some(recycle_pidl));
        anyhow::bail!("EnumObjects on recycle bin failed: {hr:?}");
    }
    let enum_id = enum_id.ok_or_else(|| anyhow::anyhow!("EnumObjects returned null enumerator"))?;

    let mut items = Vec::new();
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

        let display_name = display_name_of(&recycle_folder, pidl, SHGDN_INFOLDER).unwrap_or_else(
            |_| display_name_of(&recycle_folder, pidl, SHGDN_FORPARSING).unwrap_or_default(),
        );
        let shell_path = display_name_of(&recycle_folder, pidl, SHGDN_FORPARSING)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(&display_name));

        items.push(RecycleBinEntry {
            display_name,
            shell_path,
            size: None,
            modified: None,
        });

        ILFree(Some(pidl));
    }

    ILFree(Some(recycle_pidl));
    Ok(items)
}
