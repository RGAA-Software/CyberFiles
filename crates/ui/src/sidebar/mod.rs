mod cache;
mod data;
mod menu_with_drop;
mod model;
mod view;

pub use cache::{build_sidebar_sections_cached, sidebar_cache_key};
pub use model::{SidebarEntry, SidebarSection, SidebarSectionKind};
pub use view::{navigation_matches, render_sidebar};
