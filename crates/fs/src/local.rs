use std::path::Path;

use crate::item::{should_include_item, DirectoryReadOptions, FileItem};
use crate::sort::{sort_items, SortPreferences};

pub fn read_directory(
    path: impl AsRef<Path>,
    options: DirectoryReadOptions,
) -> anyhow::Result<Vec<FileItem>> {
    let mut items = Vec::new();

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let item = FileItem::from_path(entry.path(), options)?;
        if should_include_item(&item, options) {
            items.push(item);
        }
    }

    sort_items(&mut items, SortPreferences::default());
    Ok(items)
}
