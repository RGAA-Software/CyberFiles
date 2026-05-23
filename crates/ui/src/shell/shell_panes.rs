use gpui::{prelude::*, *};
use gpui_component::{
    resizable::{h_resizable, resizable_panel},
    ActiveTheme as _,
};

use crate::shell::navigation::NavigationTarget;
use crate::shell::PaneShell;
use cyberfiles_fs::home_navigation_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSide {
    Primary,
    Secondary,
}

pub struct ShellPanes {
    primary: Entity<PaneShell>,
    secondary: Entity<PaneShell>,
    dual_pane: bool,
    active: PaneSide,
}

impl ShellPanes {
    pub fn new(cx: &mut Context<Self>, target: NavigationTarget) -> Self {
        let secondary_path = match &target {
            NavigationTarget::Path(path) => path.clone(),
            _ => home_navigation_path(),
        };
        Self {
            primary: cx.new(|cx| PaneShell::new(cx, target)),
            secondary: cx.new(|cx| PaneShell::new(cx, NavigationTarget::Path(secondary_path))),
            dual_pane: false,
            active: PaneSide::Primary,
        }
    }

    pub fn dual_pane(&self) -> bool {
        self.dual_pane
    }

    pub fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        self.dual_pane = !self.dual_pane;
        if self.dual_pane {
            self.active = PaneSide::Primary;
        }
        cx.notify();
    }

    pub fn set_active(&mut self, side: PaneSide, cx: &mut Context<Self>) {
        if self.active != side {
            self.active = side;
            cx.notify();
        }
    }

    pub fn active_pane(&self) -> Entity<PaneShell> {
        match (self.dual_pane, self.active) {
            (true, PaneSide::Secondary) => self.secondary.clone(),
            _ => self.primary.clone(),
        }
    }

    pub fn secondary(&self) -> Entity<PaneShell> {
        self.secondary.clone()
    }

    pub fn navigate_active(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        self.active_pane().update(cx, |shell, cx| {
            shell.navigate(target, cx);
        });
        cx.notify();
    }
}

impl Render for ShellPanes {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.dual_pane {
            return div()
                .id("shell-pane-single")
                .size_full()
                .min_h_0()
                .child(self.active_pane())
                .into_any_element();
        }

        let active = self.active;
        let primary = self.primary.clone();
        let secondary = self.secondary.clone();

        h_resizable("shell-panes")
            .child(
                resizable_panel()
                    .flex_1()
                    .child(
                        div()
                            .size_full()
                            .min_h_0()
                            .border_2()
                            .border_color(if active == PaneSide::Primary {
                                cx.theme().primary
                            } else {
                                cx.theme().border
                            })
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.set_active(PaneSide::Primary, cx);
                            }))
                            .child(primary),
                    ),
            )
            .child(
                resizable_panel()
                    .flex_1()
                    .child(
                        div()
                            .size_full()
                            .min_h_0()
                            .border_2()
                            .border_color(if active == PaneSide::Secondary {
                                cx.theme().primary
                            } else {
                                cx.theme().border
                            })
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.set_active(PaneSide::Secondary, cx);
                            }))
                            .child(secondary),
                    ),
            )
            .into_any_element()
    }
}
