mod actions;
mod app_menus;
mod app_shell;
pub mod navigation;
mod pane_shell;
pub mod preferences;
mod title_bar;
mod window;

pub use navigation::NavigationTarget;
pub use pane_shell::PaneShell;
pub use window::open_main_window;

pub(crate) use actions::*;
