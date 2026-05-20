mod actions;
mod app_menus;
mod app_shell;
pub mod navigation;
mod pane_shell;
mod shell_panes;
pub mod preferences;
mod title_bar;
mod window;

pub use navigation::NavigationTarget;
pub use pane_shell::PaneShell;
pub use shell_panes::{PaneSide, ShellPanes};
pub use window::open_main_window;

pub(crate) use actions::*;
