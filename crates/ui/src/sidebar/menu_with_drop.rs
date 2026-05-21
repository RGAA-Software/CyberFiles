//! Sidebar menu that wraps [`SidebarMenuItem`] rows with folder drop targets.
//!
//! Upstream `gpui-component` does not expose file drop on sidebar items; CyberFiles
//! implements that here without patching the dependency.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, StyleRefinement, Styled, Window, div,
};
use gpui_component::{
    Collapsible, StyledExt,
    sidebar::{SidebarItem, SidebarMenuItem},
    v_flex,
};

use crate::drag::DraggedFilePaths;

#[derive(Clone)]
struct FolderDropHandlers {
    on_drag_move: Rc<dyn Fn(&mut Window, &mut App)>,
    on_drop: Rc<dyn Fn(&DraggedFilePaths, &mut Window, &mut App)>,
}

#[derive(Clone)]
enum SidebarRow {
    Plain {
        item: SidebarMenuItem,
        on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    },
    Droppable {
        item: SidebarMenuItem,
        on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        handlers: FolderDropHandlers,
    },
}

/// [`gpui_component::sidebar::SidebarMenu`] equivalent with optional per-row file drop.
#[derive(Clone)]
pub struct SidebarMenuWithDrop {
    style: StyleRefinement,
    collapsed: bool,
    rows: Vec<SidebarRow>,
}

fn render_sidebar_row(
    row_id: SharedString,
    item: SidebarMenuItem,
    collapsed: bool,
    on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    drop_handlers: Option<FolderDropHandlers>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let inner = item
        .collapsed(collapsed)
        .render(row_id.clone(), window, cx)
        .into_any_element();
    let mut row_el = div().id(row_id).w_full().child(inner);
    if let Some(middle) = on_middle_click {
        row_el = row_el.on_mouse_down(MouseButton::Middle, move |_, window, cx| {
            middle(window, cx);
        });
    }
    if let Some(handlers) = drop_handlers {
        let drag_move = handlers.on_drag_move.clone();
        let drop = handlers.on_drop.clone();
        row_el = row_el
            .on_drag_move::<DraggedFilePaths>(move |_, window, cx| {
                drag_move(window, cx);
            })
            .on_drop(move |paths: &DraggedFilePaths, window, cx| {
                drop(paths, window, cx);
            });
    }
    row_el.into_any_element()
}

impl SidebarMenuWithDrop {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            collapsed: false,
            rows: Vec::new(),
        }
    }

    pub fn child(mut self, item: impl Into<SidebarMenuItem>) -> Self {
        self.push_child(item, None);
        self
    }

    pub fn children(
        mut self,
        children: impl IntoIterator<Item = impl Into<SidebarMenuItem>>,
    ) -> Self {
        for child in children {
            self.push_child(child, None);
        }
        self
    }

    pub fn push_child(
        &mut self,
        item: impl Into<SidebarMenuItem>,
        on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    ) {
        self.rows.push(SidebarRow::Plain {
            item: item.into(),
            on_middle_click,
        });
    }

    pub fn push_child_with_folder_drop(
        &mut self,
        item: impl Into<SidebarMenuItem>,
        on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        on_drag_move: impl Fn(&mut Window, &mut App) + 'static,
        on_drop: impl Fn(&DraggedFilePaths, &mut Window, &mut App) + 'static,
    ) {
        self.rows.push(SidebarRow::Droppable {
            item: item.into(),
            on_middle_click,
            handlers: FolderDropHandlers {
                on_drag_move: Rc::new(on_drag_move),
                on_drop: Rc::new(on_drop),
            },
        });
    }
}

impl Collapsible for SidebarMenuWithDrop {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl Styled for SidebarMenuWithDrop {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl SidebarItem for SidebarMenuWithDrop {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let id = id.into();

        v_flex()
            .w_full()
            .gap_2()
            .refine_style(&self.style)
            .children(self.rows.into_iter().enumerate().map(|(ix, row)| {
                let row_id = SharedString::from(format!("{}-{}", id, ix));
                match row {
                    SidebarRow::Plain {
                        item,
                        on_middle_click,
                    } => render_sidebar_row(
                        row_id,
                        item,
                        self.collapsed,
                        on_middle_click,
                        None,
                        window,
                        cx,
                    ),
                    SidebarRow::Droppable {
                        item,
                        on_middle_click,
                        handlers,
                    } => render_sidebar_row(
                        row_id,
                        item,
                        self.collapsed,
                        on_middle_click,
                        Some(handlers),
                        window,
                        cx,
                    ),
                }
            }))
    }
}
