use std::path::{Path, PathBuf};
use std::rc::Rc;

use cyberfiles_fs::{
    breadcrumb_dropdown_entries, breadcrumb_visible_layout_for_width, BreadcrumbMenuSection,
    PathBreadcrumb,
};
use gpui::{prelude::*, *};
use     gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenu, PopupMenuItem},
    ActiveTheme as _, Icon, IconName, Sizable as _,
};
use rust_i18n::t;

use crate::app_state::AppNavigation;
use crate::file_browser::DraggedFilePaths;

/// Files-style path breadcrumb: home root + unified segment blocks (label + chevron), optional ellipsis.
#[derive(IntoElement)]
pub struct PathBreadcrumbBar {
    show_root: bool,
    segments: Vec<PathBreadcrumb>,
    available_width: f32,
    show_hidden: bool,
    working_directory: Option<PathBuf>,
    root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
    on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_home: Rc<dyn Fn(&mut Window, &mut App)>,
    on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
    on_drag_hover: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
}

impl PathBreadcrumbBar {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        show_root: bool,
        segments: Vec<PathBreadcrumb>,
        available_width: f32,
        show_hidden: bool,
        working_directory: Option<PathBuf>,
        root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
        on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_home: Rc<dyn Fn(&mut Window, &mut App)>,
        on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
        on_drag_hover: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    ) -> Self {
        Self {
            show_root,
            segments,
            available_width,
            show_hidden,
            working_directory,
            root_menu,
            on_navigate,
            on_navigate_new_tab,
            on_home,
            on_drop_paths,
            on_drag_hover,
        }
    }
}

impl RenderOnce for PathBreadcrumbBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let layout = breadcrumb_visible_layout_for_width(
            &self.segments,
            self.available_width,
            self.show_root,
        );
        let on_navigate = self.on_navigate.clone();
        let on_navigate_new_tab = self.on_navigate_new_tab.clone();
        let on_home = self.on_home.clone();
        let on_drop_paths = self.on_drop_paths.clone();
        let on_drag_hover = self.on_drag_hover.clone();
        let root_menu = self.root_menu.clone();
        let show_hidden = self.show_hidden;
        let working_directory = self.working_directory.clone();

        let mut bar = h_flex()
            .id("path-breadcrumb-bar")
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .items_center()
            .overflow_hidden();

        if self.show_root {
            bar = bar.child(render_root_item(on_home, root_menu, cx));
        }

        if layout.hidden_prefix_len > 0 {
            let collapsed: Vec<PathBreadcrumb> = self.segments[..layout.hidden_prefix_len].to_vec();
            bar = bar.child(render_ellipsis_item(
                collapsed,
                on_navigate.clone(),
                on_navigate_new_tab.clone(),
                cx,
            ));
        }

        for &index in &layout.visible_indices {
            let Some(crumb) = self.segments.get(index) else {
                continue;
            };
            let is_last = index + 1 == self.segments.len();
            let show_chevron = !is_last;
            bar = bar.child(render_path_segment(
                index,
                crumb.clone(),
                show_chevron,
                is_last,
                show_hidden,
                working_directory.as_deref(),
                on_navigate.clone(),
                on_navigate_new_tab.clone(),
                on_drop_paths.clone(),
                on_drag_hover.clone(),
                cx,
            ));
        }

        bar
    }
}

fn render_root_item(
    on_home: Rc<dyn Fn(&mut Window, &mut App)>,
    root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
    cx: &App,
) -> impl IntoElement {
    let root_menu_builder = root_dropdown_menu_builder(root_menu);
    let home_tip = t!("omnibar.breadcrumb.root_tooltip").to_string();
    let chevron_tip = t!("omnibar.breadcrumb.chevron_tooltip").to_string();
    h_flex()
        .id("breadcrumb-root")
        .items_center()
        .rounded_l(cx.theme().radius)
        .hover(|s| s.bg(cx.theme().secondary))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            Button::new("breadcrumb-root-home")
                .xsmall()
                .ghost()
                .icon(IconName::LayoutDashboard)
                .tooltip(home_tip)
                .on_click(move |_, window, cx| on_home(window, cx)),
        )
        .child(
            Button::new("breadcrumb-root-chevron")
                .xsmall()
                .ghost()
                .child(
                    Icon::new(IconName::ChevronRight)
                        .small()
                        .rotate(percentage(90. / 360.)),
                )
                .tooltip(chevron_tip)
                .dropdown_menu_with_anchor(Anchor::BottomLeft, root_menu_builder),
        )
}

fn render_ellipsis_item(
    collapsed: Vec<PathBreadcrumb>,
    on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    _on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement {
    let menu_builder = ellipsis_dropdown_menu_builder(collapsed, on_navigate);
    let tip = t!("omnibar.breadcrumb.ellipsis_tooltip").to_string();
    h_flex()
        .id("breadcrumb-ellipsis")
        .items_center()
        .rounded(cx.theme().radius)
        .hover(|s| s.bg(cx.theme().secondary))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
        .child(
            Button::new("breadcrumb-ellipsis-button")
                .xsmall()
                .ghost()
                .label("…")
                .tooltip(tip)
                .dropdown_menu_with_anchor(Anchor::BottomLeft, menu_builder),
        )
}

fn render_path_segment(
    index: usize,
    crumb: PathBreadcrumb,
    show_chevron: bool,
    is_last: bool,
    show_hidden: bool,
    working_directory: Option<&Path>,
    on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
    on_drag_hover: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement {
    let path_nav = crumb.path.clone();
    let path_menu = crumb.path.clone();
    let label = crumb.label.clone();
    let tooltip = crumb.path.display().to_string();
    let menu_builder =
        segment_dropdown_menu_builder(path_menu, show_hidden, working_directory.map(Path::to_path_buf));
    let navigate = on_navigate.clone();
    let new_tab = on_navigate_new_tab.clone();
    let drop_target = path_nav.clone();
    let hover_target = path_nav.clone();
    let drop = on_drop_paths.clone();
    let hover = on_drag_hover.clone();
    let chevron_tip = t!("omnibar.breadcrumb.chevron_tooltip").to_string();
    let is_dir = path_nav.is_dir();

    let mut segment = h_flex()
        .id(("breadcrumb-segment", index))
        .items_center()
        .rounded(cx.theme().radius)
        .hover(|s| s.bg(cx.theme().secondary))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Middle, {
            let path = path_nav.clone();
            let new_tab = new_tab.clone();
            move |_, window, cx| {
                cx.stop_propagation();
                if !is_last {
                    new_tab(path.clone(), window, cx);
                }
            }
        })
        .child({
            let navigate = navigate.clone();
            Button::new(("breadcrumb-segment-label", index))
                .xsmall()
                .ghost()
                .label(label)
                .tooltip(tooltip)
                .on_click(move |_, window, cx| {
                    if is_last {
                        return;
                    }
                    navigate(path_nav.clone(), window, cx);
                })
        });

    if show_chevron {
        segment = segment.child(
            Button::new(("breadcrumb-segment-chevron", index))
                .xsmall()
                .ghost()
                .child(
                    Icon::new(IconName::ChevronRight)
                        .small()
                        .rotate(percentage(90. / 360.)),
                )
                .tooltip(chevron_tip)
                .dropdown_menu_with_anchor(Anchor::BottomLeft, menu_builder),
        );
    }

    if is_dir && !is_last {
        segment = segment
            .on_drag_move::<DraggedFilePaths>({
                let hover = hover.clone();
                let hover_target = hover_target.clone();
                move |_, window, cx| {
                    hover(hover_target.clone(), window, cx);
                }
            })
            .on_drop({
                let drop = drop.clone();
                let drop_target = drop_target.clone();
                move |paths: &DraggedFilePaths, window, cx| {
                    drop(drop_target.clone(), paths.0.clone(), window, cx);
                }
            });
    } else if is_dir {
        segment = segment.on_drop({
            let drop = drop.clone();
            let drop_target = drop_target.clone();
            move |paths: &DraggedFilePaths, window, cx| {
                drop(drop_target.clone(), paths.0.clone(), window, cx);
            }
        });
    }

    segment
}

fn segment_dropdown_menu_builder(
    path: PathBuf,
    show_hidden: bool,
    working_directory: Option<PathBuf>,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _, _| {
        let entries = breadcrumb_dropdown_entries(
            &path,
            show_hidden,
            working_directory.as_deref(),
        );
        let mut menu = menu.scrollable(true);
        if entries.is_empty() {
            menu.item(
                PopupMenuItem::new(t!("omnibar.breadcrumb.empty").to_string()).disabled(true),
            )
        } else {
            for entry in entries {
                let target = entry.path.clone();
                let entry_label = entry.label.clone();
                menu = menu.item(
                    PopupMenuItem::new(entry_label).on_click(move |_, _, cx| {
                        AppNavigation::navigate_to_path(target.clone(), cx);
                    }),
                );
            }
            menu
        }
    }
}

fn root_dropdown_menu_builder(
    root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _, _| {
        let sections = root_menu();
        let mut menu = menu.scrollable(true);
        if sections.is_empty() {
            return menu.item(
                PopupMenuItem::new(t!("omnibar.breadcrumb.empty").to_string()).disabled(true),
            );
        }
        for (section_index, section) in sections.iter().enumerate() {
            if section_index > 0 {
                menu = menu.item(PopupMenuItem::separator());
            }
            if let Some(ref heading) = section.heading {
                menu = menu.item(PopupMenuItem::label(heading.clone()));
            }
            for entry in &section.entries {
                let target = entry.path.clone();
                let entry_label = entry.label.clone();
                menu = menu.item(
                    PopupMenuItem::new(entry_label).on_click(move |_, _, cx| {
                        AppNavigation::navigate_to_path(target.clone(), cx);
                    }),
                );
            }
        }
        menu
    }
}

fn ellipsis_dropdown_menu_builder(
    collapsed: Vec<PathBreadcrumb>,
    on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _, _| {
        let mut menu = menu.scrollable(true);
        for crumb in &collapsed {
            let path = crumb.path.clone();
            let label = crumb.label.clone();
            let navigate = on_navigate.clone();
            menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                navigate(path.clone(), window, cx);
            }));
        }
        menu
    }
}
