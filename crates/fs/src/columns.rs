use std::path::{Path, PathBuf};

/// Builds the ancestor path chain for column view (e.g. `C:\` → `C:\Users` → … → current).
pub fn column_trail_for(path: &Path) -> Vec<PathBuf> {
    let mut trail = Vec::new();
    let mut acc = PathBuf::new();
    for component in path.components() {
        acc.push(component);
        if acc.as_os_str().is_empty() {
            continue;
        }
        trail.push(acc.clone());
    }
    if trail.is_empty() {
        trail.push(path.to_path_buf());
    } else if trail.last().map(|p| p.as_path()) != Some(path) {
        trail.push(path.to_path_buf());
    }
    trail
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_trail_includes_each_prefix() {
        let path = PathBuf::from(r"C:\Users\test");
        let trail = column_trail_for(&path);
        assert!(!trail.is_empty());
        assert_eq!(trail.last().map(|p| p.as_path()), Some(path.as_path()));
    }
}
