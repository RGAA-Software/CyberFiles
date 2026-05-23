mod actions;

pub use actions::ReopenClosedTabAt;
pub(crate) mod app_menus;
mod app_shell;
pub mod navigation;
mod pane_shell;
pub mod preferences;
mod shell_panes;
mod window;

pub use pane_shell::PaneShell;
pub use shell_panes::{PaneSide, ShellPanes};
pub use window::open_main_window;

pub(crate) use actions::*;
