use cyberfiles_core::{home_widget_prefs, save_home_widget_prefs, HomeWidgetPrefs};
use cyberfiles_fs::{
    file_tag_previews, list_drives, list_quick_access_entries, list_recent_files,
    load_home_file_tags, DriveInfo, FileTagPreview, QuickAccessEntry, RecentItem,
};
use gpui::{prelude::*, *};
use gpui_component::{
    menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem},
    v_flex,
};
use rust_i18n::t;

use crate::app_state::AppNavigation;
use crate::home::widgets::{load_network_entries, NetworkEntry};

/// Loaded Home dashboard data (Files `RefreshWidgetProperties` snapshot).
#[derive(Clone)]
pub struct HomeSnapshot {
    pub quick_access: Vec<QuickAccessEntry>,
    pub drives: Vec<DriveInfo>,
    pub network: Vec<NetworkEntry>,
    pub tag_previews: Vec<FileTagPreview>,
    pub recent: Vec<RecentItem>,
}

impl HomeSnapshot {
    fn load() -> Self {
        let tags = load_home_file_tags();
        Self {
            quick_access: list_quick_access_entries(),
            drives: list_drives(),
            network: load_network_entries(),
            tag_previews: file_tag_previews(&tags),
            recent: list_recent_files(),
        }
    }
}

pub struct HomePage {
    pub(super) prefs: HomeWidgetPrefs,
    snapshot: Option<HomeSnapshot>,
    loading: bool,
    load_generation: u64,
}

impl HomePage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            prefs: home_widget_prefs(),
            snapshot: None,
            loading: false,
            load_generation: 0,
        };
        page.schedule_load(cx);
        page
    }

    pub fn view(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(cx))
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.snapshot = None;
        self.schedule_load(cx);
    }

    fn schedule_load(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.load_generation = self.load_generation.wrapping_add(1);
        let generation = self.load_generation;
        cx.spawn(async move |page, cx| {
            let snapshot = cx
                .background_spawn(async move { HomeSnapshot::load() })
                .await;
            let _ = page.update(cx, |page, cx| {
                if page.load_generation != generation {
                    return;
                }
                page.snapshot = Some(snapshot);
                page.loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_expanded(&mut self, section: &str, cx: &mut Context<Self>) {
        match section {
            "quick_access" => self.prefs.quick_access_expanded = !self.prefs.quick_access_expanded,
            "drives" => self.prefs.drives_expanded = !self.prefs.drives_expanded,
            "network" => self.prefs.network_expanded = !self.prefs.network_expanded,
            "file_tags" => self.prefs.file_tags_expanded = !self.prefs.file_tags_expanded,
            "recent" => self.prefs.recent_expanded = !self.prefs.recent_expanded,
            _ => {}
        }
        let _ = save_home_widget_prefs(&self.prefs);
        cx.notify();
    }

    fn toggle_widget_visible(&mut self, key: &str, cx: &mut Context<Self>) {
        match key {
            "quick_access" => self.prefs.show_quick_access = !self.prefs.show_quick_access,
            "drives" => self.prefs.show_drives = !self.prefs.show_drives,
            "network" => self.prefs.show_network = !self.prefs.show_network,
            "file_tags" => self.prefs.show_file_tags = !self.prefs.show_file_tags,
            "recent" => self.prefs.show_recent = !self.prefs.show_recent,
            _ => {}
        }
        let _ = save_home_widget_prefs(&self.prefs);
        cx.notify();
    }

}

fn build_page_context_menu(menu: PopupMenu, prefs: &HomeWidgetPrefs) -> PopupMenu {
    let mut menu = menu;
    let items = [
        (
            "quick_access",
            t!("home.widget.quick_access"),
            prefs.show_quick_access,
        ),
        ("drives", t!("home.widget.drives"), prefs.show_drives),
        ("network", t!("home.widget.network"), prefs.show_network),
        ("file_tags", t!("home.widget.tags"), prefs.show_file_tags),
        ("recent", t!("home.widget.recent"), prefs.show_recent),
    ];
    for (key, label, checked) in items {
        let suffix = if checked { " ✓" } else { "" };
        let text = format!("{label}{suffix}");
        let key = key.to_string();
        menu = menu.item(PopupMenuItem::new(text).on_click(move |_, _, cx| {
            let mut prefs = home_widget_prefs();
            match key.as_str() {
                "quick_access" => prefs.show_quick_access = !prefs.show_quick_access,
                "drives" => prefs.show_drives = !prefs.show_drives,
                "network" => prefs.show_network = !prefs.show_network,
                "file_tags" => prefs.show_file_tags = !prefs.show_file_tags,
                "recent" => prefs.show_recent = !prefs.show_recent,
                _ => {}
            }
            let _ = save_home_widget_prefs(&prefs);
            AppNavigation::refresh_home_widgets(cx);
            cx.stop_propagation();
        }));
    }
    menu
}

impl Render for HomePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prefs = home_widget_prefs();
        if self.snapshot.is_none() && !self.loading {
            self.schedule_load(cx);
        }

        let snapshot = self.snapshot.clone().unwrap_or_else(|| HomeSnapshot {
            quick_access: Vec::new(),
            drives: Vec::new(),
            network: Vec::new(),
            tag_previews: Vec::new(),
            recent: Vec::new(),
        });

        let show_qa = self.prefs.show_quick_access;
        let show_drives = self.prefs.show_drives;
        let show_network = self.prefs.show_network;
        let show_tags = self.prefs.show_file_tags;
        let show_recent = self.prefs.show_recent;
        let menu_prefs = self.prefs.clone();

        v_flex()
            .id("home-page")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .p_4()
            .gap_3()
            .context_menu(move |menu, _window, _cx| build_page_context_menu(menu, &menu_prefs))
            .when(self.loading && self.snapshot.is_none(), |page| {
                page.child(div().child(t!("home.loading")))
            })
            .when(show_qa, |page| {
                page.child(self.render_quick_access_widget(
                    window,
                    cx,
                    &snapshot.quick_access,
                ))
            })
            .when(show_drives, |page| {
                page.child(self.render_drives_widget(window, cx, &snapshot.drives))
            })
            .when(show_network, |page| {
                page.child(self.render_network_widget(window, cx, &snapshot.network))
            })
            .when(show_tags, |page| {
                page.child(self.render_file_tags_widget(window, cx, &snapshot.tag_previews))
            })
            .when(show_recent, |page| {
                page.child(self.render_recent_widget(window, cx, &snapshot.recent))
            })
    }
}
