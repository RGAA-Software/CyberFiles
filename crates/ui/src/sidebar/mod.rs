mod data;
mod model;
mod view;

pub use model::{SidebarEntry, SidebarSection, SidebarSectionKind};
pub use view::{navigation_matches, render_sidebar};
