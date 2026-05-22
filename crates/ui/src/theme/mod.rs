//! CyberFiles-owned theme catalog. Color JSON is embedded from `cyberfiles-assets`;
//! runtime always applies these themes instead of gpui-component built-in defaults.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use cyberfiles_assets::themes::{CYBERFILES_BLUE, CYBERFILES_MINT, CYBERFILES_YELLOW};
use gpui::{App, SharedString};
use gpui_component::{Theme, ThemeConfig, ThemeMode, ThemeSet};

/// Persisted `theme_name` in `settings.json` (theme set id, not light/dark variant name).
pub const DEFAULT_THEME_SET_ID: &str = "CyberFiles Blue";

static CATALOG: OnceLock<ThemeCatalog> = OnceLock::new();

/// One theme set (e.g. CyberFiles Blue) with light and dark variants.
#[derive(Debug, Clone)]
pub struct ThemeSetEntry {
    pub id: SharedString,
    pub display_name: SharedString,
    pub light: Arc<ThemeConfig>,
    pub dark: Arc<ThemeConfig>,
}

#[derive(Debug)]
pub struct ThemeCatalog {
    pub default_id: SharedString,
    sets: HashMap<SharedString, ThemeSetEntry>,
}

impl ThemeCatalog {
    pub fn global() -> &'static Self {
        CATALOG.get_or_init(Self::load_embedded)
    }

    pub fn sets(&self) -> impl Iterator<Item = &ThemeSetEntry> {
        self.sets.values()
    }

    pub fn sorted_sets(&self) -> Vec<&ThemeSetEntry> {
        let mut sets: Vec<_> = self.sets().collect();
        sets.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
        sets
    }

    pub fn get(&self, id: &str) -> Option<&ThemeSetEntry> {
        self.sets.get(id)
    }

    pub fn config_for_mode(&self, set_id: &str, mode: ThemeMode) -> Option<Arc<ThemeConfig>> {
        let entry = self.get(set_id)?;
        Some(if mode.is_dark() {
            entry.dark.clone()
        } else {
            entry.light.clone()
        })
    }

    pub fn normalize_set_id(name: &str) -> SharedString {
        let base = name
            .strip_suffix(" Light")
            .or_else(|| name.strip_suffix(" Dark"))
            .unwrap_or(name);
        match base {
            "Default" | "Default Light" | "Default Dark" => DEFAULT_THEME_SET_ID.into(),
            "CyberFiles" => DEFAULT_THEME_SET_ID.into(),
            other => other.into(),
        }
    }

    fn load_embedded() -> Self {
        let mut sets = HashMap::new();
        for json in [CYBERFILES_BLUE, CYBERFILES_MINT, CYBERFILES_YELLOW] {
            if let Ok(set) = serde_json::from_str::<ThemeSet>(json) {
                if let Some(entry) = ThemeSetEntry::from_theme_set(set) {
                    sets.insert(entry.id.clone(), entry);
                }
            }
        }
        let default_id = sets
            .keys()
            .find(|id| id.as_ref() == DEFAULT_THEME_SET_ID)
            .cloned()
            .unwrap_or_else(|| DEFAULT_THEME_SET_ID.into());
        Self { default_id, sets }
    }
}

impl ThemeSetEntry {
    fn from_theme_set(set: ThemeSet) -> Option<Self> {
        let mut light = None;
        let mut dark = None;
        for theme in set.themes {
            if theme.mode.is_dark() {
                dark = Some(Arc::new(theme));
            } else {
                light = Some(Arc::new(theme));
            }
        }
        let (light, dark) = (light?, dark?);
        Some(Self {
            id: set.name.clone(),
            display_name: set.name,
            light,
            dark,
        })
    }
}

/// Install CyberFiles themes after `gpui_component::init` (replaces active light/dark configs).
pub fn install(cx: &mut App) {
    let catalog = ThemeCatalog::global();
    let default = catalog
        .get(catalog.default_id.as_ref())
        .expect("embedded CyberFiles theme");

    if !cx.has_global::<Theme>() {
        Theme::change(ThemeMode::Light, None, cx);
    }

    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new((*default.light).clone());
    theme.dark_theme = Rc::new((*default.dark).clone());
    let mode = theme.mode;
    Theme::change(mode, None, cx);
}

/// Apply a theme set for the given appearance mode.
pub fn apply_set(set_id: &str, mode: ThemeMode, cx: &mut App) {
    let catalog = ThemeCatalog::global();
    let set_id = ThemeCatalog::normalize_set_id(set_id).to_string();
    let Some(entry) = catalog.get(&set_id) else {
        apply_set(catalog.default_id.as_ref(), mode, cx);
        return;
    };

    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new((*entry.light).clone());
    theme.dark_theme = Rc::new((*entry.dark).clone());
    Theme::change(mode, None, cx);
}

/// Dropdown options: theme set id → display label.
pub fn theme_set_options() -> Vec<(SharedString, SharedString)> {
    ThemeCatalog::global()
        .sorted_sets()
        .into_iter()
        .map(|entry| (entry.id.clone(), entry.display_name.clone()))
        .collect()
}

pub fn current_theme_set_id(cx: &App) -> SharedString {
    ThemeCatalog::normalize_set_id(Theme::global(cx).theme_name().as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_theme_sets_load() {
        let catalog = ThemeCatalog::load_embedded();
        assert_eq!(catalog.sets.len(), 3);
        for id in ["CyberFiles Blue", "CyberFiles Mint", "CyberFiles Yellow"] {
            let entry = catalog.get(id).expect(id);
            assert!(entry.light.mode == ThemeMode::Light);
            assert!(entry.dark.mode == ThemeMode::Dark);
            assert!(entry.light.name.as_ref().contains(id));
            assert!(entry.dark.name.as_ref().contains(id));
        }
    }
}
