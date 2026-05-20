use gpui::{Action, SharedString, actions};
use gpui_component::{ThemeMode, scroll::ScrollbarShow};
use serde::Deserialize;

actions!(cyberfiles_shell, [About, Quit]);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = cyberfiles_shell, no_json)]
pub struct SelectFont(pub usize);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = cyberfiles_shell, no_json)]
pub struct SelectRadius(pub usize);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = cyberfiles_shell, no_json)]
pub struct SelectScrollbarShow(pub ScrollbarShow);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = cyberfiles_shell, no_json)]
pub struct ToggleListActiveHighlight;

#[derive(Action, Clone, PartialEq)]
#[action(namespace = cyberfiles_shell, no_json)]
pub(crate) struct SwitchTheme(pub SharedString);

#[derive(Action, Clone, PartialEq)]
#[action(namespace = cyberfiles_shell, no_json)]
pub(crate) struct SwitchThemeMode(pub ThemeMode);
