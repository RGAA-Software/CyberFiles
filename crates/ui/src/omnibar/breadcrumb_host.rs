use std::path::PathBuf;
use std::rc::Rc;

use cyberfiles_fs::{BreadcrumbMenuSection, PathBreadcrumb};
use gpui::{prelude::*, *};
use gpui_component::ElementExt as _;

use super::breadcrumb_bar::PathBreadcrumbBar;

/// Measures omnibar breadcrumb host width and feeds it into [`PathBreadcrumbBar`].
pub struct OmnibarBreadcrumbHost {
    show_root: bool,
    segments: Vec<PathBreadcrumb>,
    show_hidden: bool,
    working_directory: Option<PathBuf>,
    root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
    on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_home: Rc<dyn Fn(&mut Window, &mut App)>,
    on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
    on_drag_hover: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    on_show_full_path: Rc<dyn Fn(&mut Window, &mut App)>,
    measured_width: f32,
}

impl OmnibarBreadcrumbHost {
    pub fn set_path_context(
        &mut self,
        segments: Vec<PathBreadcrumb>,
        working_directory: Option<PathBuf>,
        show_hidden: bool,
    ) {
        self.segments = segments;
        self.working_directory = working_directory;
        self.show_hidden = show_hidden;
    }

    pub fn set_measured_width(&mut self, width: f32) {
        if width >= 1.0 {
            self.measured_width = width;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        show_root: bool,
        segments: Vec<PathBreadcrumb>,
        show_hidden: bool,
        working_directory: Option<PathBuf>,
        root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
        on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_home: Rc<dyn Fn(&mut Window, &mut App)>,
        on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
        on_drag_hover: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_show_full_path: Rc<dyn Fn(&mut Window, &mut App)>,
    ) -> Self {
        Self {
            show_root,
            segments,
            show_hidden,
            working_directory,
            root_menu,
            on_navigate,
            on_navigate_new_tab,
            on_home,
            on_drop_paths,
            on_drag_hover,
            on_show_full_path,
            measured_width: 10_000.,
        }
    }
}

impl Render for OmnibarBreadcrumbHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let host = cx.entity();
        div()
            .id("omnibar-breadcrumb-measure")
            .flex_1()
            .min_w_0()
            .on_prepaint({
                move |bounds, _, cx| {
                    let w = f32::from(bounds.size.width);
                    let _ = host.update(cx, |host, cx| {
                        if w >= 1.0 && (host.measured_width - w).abs() > 1.5 {
                            host.measured_width = w;
                            cx.notify();
                        }
                    });
                }
            })
            .child(PathBreadcrumbBar::new(
                self.show_root,
                self.segments.clone(),
                self.measured_width,
                self.show_hidden,
                self.working_directory.clone(),
                self.root_menu.clone(),
                self.on_navigate.clone(),
                self.on_navigate_new_tab.clone(),
                self.on_home.clone(),
                self.on_drop_paths.clone(),
                self.on_drag_hover.clone(),
                self.on_show_full_path.clone(),
            ))
    }
}
