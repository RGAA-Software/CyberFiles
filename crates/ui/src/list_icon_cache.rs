//! Per-extension Shell list icons (Files `IconCacheService` — dummy path + extension key).

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, OnceLock, RwLock};

use cyberfiles_fs::{FileItem, FileItemKind};
use cyberfiles_platform_windows::{self as platform};

/// Cache key (`:folder:`, `.zip`, `:noext:`) — matches Files `IconCacheService`.
pub type ListIconKey = String;

fn cache() -> &'static RwLock<HashMap<(ListIconKey, u32), Arc<Vec<u8>>>> {
    static CACHE: OnceLock<RwLock<HashMap<(ListIconKey, u32), Arc<Vec<u8>>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Extension / kind key for a row (no Shell I/O).
pub fn list_icon_key(item: &FileItem) -> ListIconKey {
    match item.kind {
        FileItemKind::Folder => ":folder:".into(),
        FileItemKind::Symlink => ":symlink:".into(),
        _ => item
            .extension
            .as_ref()
            .filter(|e| !e.is_empty())
            .map(|e| format!(".{}", e.to_ascii_lowercase()))
            .unwrap_or_else(|| ":noext:".into()),
    }
}

/// Unique keys for all rows in a directory listing.
pub fn list_icon_keys_for_items(items: &[FileItem]) -> Vec<ListIconKey> {
    items.iter().map(list_icon_key).collect::<BTreeSet<_>>().into_iter().collect()
}

/// Cached PNG for a list icon key, if already loaded.
pub fn list_icon_png_cached(key: &ListIconKey, size_px: u32) -> Option<Arc<Vec<u8>>> {
    cache().read().ok()?.get(&(key.clone(), size_px)).cloned()
}

fn store_list_icon(key: ListIconKey, size_px: u32, png: Vec<u8>) {
    if png.is_empty() {
        return;
    }
    if let Ok(mut guard) = cache().write() {
        guard.insert((key, size_px), Arc::new(png));
    }
}

fn load_one(key: ListIconKey, size_px: u32) {
    if list_icon_png_cached(&key, size_px).is_some() {
        return;
    }
    match platform::shell_icon_png_for_list_key(&key, size_px) {
        Ok(png) if !png.is_empty() => store_list_icon(key, size_px, png),
        Ok(_) | Err(_) => {
            eprintln!("[list-icon] failed to load key={key:?}");
        }
    }
}

/// Load each missing extension icon once (background thread; Files `STATask` per icon).
pub fn warm_list_icons(keys: Vec<ListIconKey>, size_px: u32) {
    for key in keys {
        load_one(key, size_px);
    }
}
