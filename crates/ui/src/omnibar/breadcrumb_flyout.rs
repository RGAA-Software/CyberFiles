//! Breadcrumb chevron flyout — window-space menu position (Files path dropdown placement).
//!
//! Popovers inside nested omnibar `relative` hosts treat layout-local bounds as window
//! coordinates and snap menus to y=0. This mirrors [`ContextMenu`] deferred anchoring at
//! [`MouseDownEvent::position`] in window space.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    anchored, deferred, div, percentage, point, prelude::FluentBuilder, px, Anchor, AnyElement,
    App, Context, DismissEvent, Element, ElementId, Entity, Focusable, GlobalElementId, Hitbox,
    HitboxBehavior, InspectorElementId, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Pixels, Point, RenderOnce, SharedString, StyleRefinement,
    Styled, Subscription, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    menu::PopupMenu,
    Icon, IconName, Selectable, Sizable as _,
};

/// Chevron trigger; rotates 90° while flyout is open (Files `ChevronNormalOn`).
#[derive(IntoElement)]
struct BreadcrumbChevronTrigger {
    id: ElementId,
    tooltip: SharedString,
    open: bool,
}

impl RenderOnce for BreadcrumbChevronTrigger {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        Button::new(self.id)
            .xsmall()
            .ghost()
            .tooltip(self.tooltip)
            .selected(self.open)
            .child(
                Icon::new(IconName::ChevronRight)
                    .small()
                    .when(self.open, |icon| icon.rotate(percentage(90. / 360.))),
            )
    }
}

/// Left-click flyout: menu below the chevron; trigger icon rotates when open.
pub(crate) struct BreadcrumbFlyout {
    id: ElementId,
    button_id: ElementId,
    tooltip: SharedString,
    menu: Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>,
    _ignore_style: StyleRefinement,
    anchor: Anchor,
}

impl BreadcrumbFlyout {
    pub fn new(
        id: impl Into<ElementId>,
        button_id: impl Into<ElementId>,
        tooltip: impl Into<SharedString>,
        menu: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            button_id: button_id.into(),
            tooltip: tooltip.into(),
            menu: Rc::new(menu),
            _ignore_style: StyleRefinement::default(),
            anchor: Anchor::TopLeft,
        }
    }

    fn with_element_state<R>(
        &mut self,
        id: &GlobalElementId,
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(&mut Self, &mut FlyoutState, &mut Window, &mut App) -> R,
    ) -> R {
        window.with_optional_element_state::<FlyoutState, _>(Some(id), |element_state, window| {
            let mut element_state = element_state.unwrap().unwrap_or_default();
            let result = f(self, &mut element_state, window, cx);
            (result, Some(element_state))
        })
    }
}

impl ParentElement for BreadcrumbFlyout {
    fn extend(&mut self, _elements: impl IntoIterator<Item = AnyElement>) {}
}

impl Styled for BreadcrumbFlyout {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self._ignore_style
    }
}

impl IntoElement for BreadcrumbFlyout {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct FlyoutSharedState {
    menu_view: Option<Entity<PopupMenu>>,
    open: bool,
    position: Point<Pixels>,
    trigger_size: gpui::Size<Pixels>,
    _subscription: Option<Subscription>,
}

pub(crate) struct FlyoutState {
    element: Option<AnyElement>,
    shared_state: Rc<RefCell<FlyoutSharedState>>,
}

impl Default for FlyoutState {
    fn default() -> Self {
        Self {
            element: None,
            shared_state: Rc::new(RefCell::new(FlyoutSharedState {
                menu_view: None,
                open: false,
                position: Point::default(),
                trigger_size: gpui::Size::default(),
                _subscription: None,
            })),
        }
    }
}

impl Element for BreadcrumbFlyout {
    type RequestLayoutState = FlyoutState;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let anchor = self.anchor;

        self.with_element_state(id.unwrap(), window, cx, |this, state, window, cx| {
            let (position, open) = {
                let shared = state.shared_state.borrow();
                (shared.position, shared.open)
            };
            let menu_view = state.shared_state.borrow().menu_view.clone();
            let mut menu_element = None;
            if open {
                let has_menu_item = menu_view
                    .as_ref()
                    .map(|menu| !menu.read(cx).is_empty())
                    .unwrap_or(false);

                if has_menu_item {
                    menu_element = Some(
                        deferred(
                            anchored().child(
                                div()
                                    .w(window.bounds().size.width)
                                    .h(window.bounds().size.height)
                                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                                    .child(
                                        anchored()
                                            .position(position)
                                            .snap_to_window_with_margin(px(8.))
                                            .anchor(anchor)
                                            .when_some(menu_view, |this, menu| {
                                                if !menu
                                                    .focus_handle(cx)
                                                    .contains_focused(window, cx)
                                                {
                                                    menu.focus_handle(cx).focus(window, cx);
                                                }
                                                this.child(menu.clone())
                                            }),
                                    ),
                            ),
                        )
                        .with_priority(1)
                        .into_any(),
                    );
                }
            }

            let trigger = BreadcrumbChevronTrigger {
                id: this.button_id.clone(),
                tooltip: this.tooltip.clone(),
                open,
            };

            let mut element = div()
                .child(trigger)
                .children(menu_element)
                .into_any_element();

            let layout_id = element.request_layout(window, cx);

            (
                layout_id,
                FlyoutState {
                    element: Some(element),
                    shared_state: state.shared_state.clone(),
                },
            )
        })
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: gpui::Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        request_layout.shared_state.borrow_mut().trigger_size = bounds.size;
        if let Some(element) = &mut request_layout.element {
            element.prepaint(window, cx);
        }
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui::Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(element) = &mut request_layout.element {
            element.paint(window, cx);
        }

        let builder = self.menu.clone();

        self.with_element_state(id.unwrap(), window, cx, |_, state, window, _| {
            let shared_state = state.shared_state.clone();
            let hitbox = hitbox.clone();

            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase.bubble()
                    && event.button == MouseButton::Left
                    && hitbox.is_hovered(window)
                {
                    cx.stop_propagation();
                    let already_open = shared_state.borrow().open;
                    if already_open {
                        shared_state.borrow_mut().open = false;
                        window.refresh();
                        return;
                    }

                    let trigger_size = shared_state.borrow().trigger_size;
                    let position = point(
                        event.position.x - trigger_size.width / 2.,
                        event.position.y + trigger_size.height / 2. + px(2.),
                    );
                    {
                        let mut shared = shared_state.borrow_mut();
                        shared.menu_view = None;
                        shared._subscription = None;
                        shared.position = position;
                        shared.open = true;
                    }

                    window.defer(cx, {
                        let shared_state = shared_state.clone();
                        let builder = builder.clone();
                        move |window, cx| {
                            let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
                                builder(menu, window, cx)
                            });

                            let subscription = window.subscribe(&menu, cx, {
                                let shared_state = shared_state.clone();
                                move |_, _: &DismissEvent, window, _cx| {
                                    shared_state.borrow_mut().open = false;
                                    window.refresh();
                                }
                            });

                            {
                                let mut shared = shared_state.borrow_mut();
                                shared.menu_view = Some(menu.clone());
                                shared._subscription = Some(subscription);
                                window.refresh();
                            }
                        }
                    });
                }
            });
        });
    }
}
