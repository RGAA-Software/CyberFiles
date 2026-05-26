use super::*;

impl FileBrowser {
    pub(super) fn dismiss_main_page_path_edit_if_active(cx: &mut Context<Self>) {
        if let Some(nav) = cx.try_global::<AppNavigation>() {
            nav.main_page().update(cx, |page, cx| {
                page.dismiss_omnibar_path_edit(cx);
            });
        }
    }

    pub(super) fn file_list(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.schedule_list_icon_warm(window, cx);
        match self.view_mode {
            ViewMode::Details => self.details_table(window, cx).into_any_element(),
            ViewMode::List => self.list_view(window, cx).into_any_element(),
            ViewMode::Grid => self.grid_view(window, cx).into_any_element(),
            ViewMode::Cards => self.cards_view(window, cx).into_any_element(),
            ViewMode::Columns => self.columns_view(window, cx).into_any_element(),
        }
    }

    fn columns_view(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let columns = self
            .column_trail
            .iter()
            .enumerate()
            .zip(self.column_listings.iter())
            .zip(self.column_scroll_handles.iter())
            .map(|(((col_index, col_path), items), scroll_handle)| {
                let title = col_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| col_path.to_string_lossy().to_string());
                let selected_name = self.column_selection_name(col_index);
                let item_count = items.len();
                let item_sizes = Rc::new(vec![COLUMN_ROW_SIZE; item_count.max(1)]);

                let is_active = self.active_column_index == Some(col_index);

                v_flex()
                    .id(("files-column", col_index))
                    .w(COLUMN_WIDTH)
                    .flex_none()
                    .h_full()
                    .min_h_0()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.cancel_rename_if_active(cx);
                            Self::dismiss_main_page_path_edit_if_active(cx);
                            this.activate_column(col_index, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        h_flex()
                            .h_8()
                            .px_2()
                            .flex_none()
                            .items_center()
                            .bg(if is_active {
                                cx.theme().primary
                            } else {
                                cx.theme().muted
                            })
                            .text_xs()
                            .text_color(if is_active {
                                cx.theme().primary_foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(title),
                    )
                    .child(
                        v_flex()
                            .id(("files-column-content", col_index))
                            .flex_1()
                            .min_h_0()
                            .size_full()
                            .overflow_hidden()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    this.cancel_rename_if_active(cx);
                                    Self::dismiss_main_page_path_edit_if_active(cx);
                                    this.begin_sweep_selection(
                                        SweepSelectionSurface::Column(col_index),
                                        event.position,
                                        event.modifiers,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }),
                            )
                            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                                this.update_sweep_pointer(
                                    SweepSelectionSurface::Column(col_index),
                                    event.position,
                                    cx,
                                );
                            }))
                            .on_prepaint({
                                let entity = cx.entity().clone();
                                move |bounds, _window, cx| {
                                    let _ = entity.update(cx, |this, _cx| {
                                        this.column_sweep_bounds.insert(col_index, bounds);
                                    });
                                }
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| {
                                    this.finish_sweep_selection();
                                }),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| {
                                    this.finish_sweep_selection();
                                }),
                            )
                            .child(
                                v_virtual_list(
                                    cx.entity().clone(),
                                    format!("files-column-virtual-list-{col_index}"),
                                    item_sizes,
                                    move |this, visible_range, window, cx| {
                                        let has_explicit_column_selection = this
                                            .column_listings
                                            .get(col_index)
                                            .is_some_and(|items| {
                                                items.iter().any(|item| {
                                                    this.selected_paths.contains(&item.path)
                                                })
                                            })
                                            || this
                                                .column_selected_path
                                                .as_ref()
                                                .is_some_and(|(selected_col, _)| {
                                                    *selected_col == col_index
                                                });
                                        visible_range
                                            .filter_map(|index| {
                                                let item = this.column_listings.get(col_index)?.get(index)?.clone();
                                                let is_selected = if has_explicit_column_selection {
                                                    if item.kind == FileItemKind::Folder {
                                                        this.selected_paths.contains(&item.path)
                                                    } else {
                                                        this.column_selected_path
                                                            == Some((col_index, item.path.clone()))
                                                            || this.selected_paths.contains(&item.path)
                                                    }
                                                } else if item.kind == FileItemKind::Folder {
                                                    selected_name.as_deref()
                                                        == Some(item.display_name.as_str())
                                                        || this.selected_paths.contains(&item.path)
                                                } else {
                                                    this.column_selected_path
                                                        == Some((col_index, item.path.clone()))
                                                        || this.selected_paths.contains(&item.path)
                                                };
                                                let drag_paths = vec![item.path.clone()];
                                                let rename_input = this.renaming_input_for(&item.path);
                                                Some(Self::column_cell(
                                                    window,
                                                    col_index,
                                                    index,
                                                    item,
                                                    is_selected,
                                                    drag_paths,
                                                    rename_input,
                                                    cx,
                                                ))
                                            })
                                            .collect()
                                    },
                                )
                                .track_scroll(scroll_handle),
                            )
                            .when_some(self.render_column_sweep_overlay(col_index, cx), |this, overlay| {
                                this.child(overlay)
                            }),
                    )
                    .scrollbar(scroll_handle, ScrollbarAxis::Vertical)
            })
            .collect::<Vec<_>>();

        v_flex()
            .id("files-columns-shell")
            .size_full()
            .flex_1()
            .min_h_0()
            .on_prepaint({
                let entity = cx.entity().clone();
                move |bounds, window, cx| {
                    let changed = entity.update(cx, |this, cx| {
                        this.update_columns_horizontal_scrollbar_state(bounds, cx)
                    });
                    if changed {
                        window.refresh();
                    }
                }
            })
            .child(
                h_flex()
                    .id("files-columns-wrap")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .items_start()
                    .overflow_x_scroll()
                    .map(|mut this| {
                        this.style().restrict_scroll_to_axis = Some(true);
                        this
                    })
                    .track_scroll(&self.columns_horizontal_scroll_handle)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.cancel_rename_if_active(cx);
                            Self::dismiss_main_page_path_edit_if_active(cx);
                            this.active_column_index = None;
                            this.clear_selection();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.cancel_rename_if_active(cx);
                            Self::dismiss_main_page_path_edit_if_active(cx);
                            this.clear_selection();
                            this.set_context_menu_extended_verbs(event.modifiers.shift);
                            this.open_context_menu(event.position, window, cx);
                        }),
                    )
                    .children(columns),
            )
            .when(self.columns_horizontal_overflow, |this| {
                this.child(
                    Scrollbar::horizontal(&self.columns_horizontal_scroll_handle)
                        .id("files-columns-horizontal-scrollbar")
                        .scrollbar_show(ScrollbarShow::Always),
                )
            })
    }

    fn column_cell(
        window: &mut Window,
        col_index: usize,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let kind = item.kind;
        let name = item.display_name.clone();
        let item_click = item.clone();
        h_flex()
            .id(format!("file-column-row-{col_index}-{name}"))
            .w_full()
            .h_8()
            .flex_none()
            .px_2()
            .gap_2()
            .items_center()
            .text_sm()
            .text_color(cx.theme().foreground)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    if event.modifiers.shift || event.modifiers.secondary() {
                        this.begin_sweep_selection(
                            SweepSelectionSurface::Column(col_index),
                            event.position,
                            event.modifiers,
                            cx,
                        );
                    }
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.focus(&this.focus_handle, cx);
                this.cancel_rename_if_active(cx);
                Self::dismiss_main_page_path_edit_if_active(cx);
                if event.modifiers().shift || event.modifiers().secondary() {
                    this.handle_column_item_click(
                        col_index,
                        index,
                        &item_click,
                        event.modifiers(),
                        cx,
                    );
                } else if kind == FileItemKind::Folder {
                    this.select_column_item(col_index, &item_click, cx);
                } else if event.click_count() == 2 {
                    this.open_item(item_click.path.clone(), kind, cx);
                } else {
                    this.handle_column_item_click(
                        col_index,
                        index,
                        &item_click,
                        event.modifiers(),
                        cx,
                    );
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_column_context_menu_target(col_index, index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(
                    SweepSelectionSurface::Column(col_index),
                    index,
                    cx,
                );
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                |paths, _offset, _window, cx| {
                    let label = if paths.0.len() == 1 {
                        paths.0[0]
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| t!("files.type.file").to_string())
                    } else {
                        format!("{} {}", paths.0.len(), t!("files.status.items"))
                    };
                    cx.new(|_| DragPathPreview {
                        label: label.into(),
                    })
                },
            )
            .child(
                div()
                    .w(px(20.))
                    .flex_none()
                    .child(Self::row_list_icon(&item, px(16.), window)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(name).into_any_element(),
                            |input| Self::inline_name_editor(input, false, cx),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn details_table(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("files-details-table")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .child(
                h_flex()
                    .h_8()
                    .px_3()
                    .gap_3()
                    .items_center()
                    .bg(cx.theme().muted)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().w(px(28.)).flex_none())
                    .child(div().flex_1().min_w_0().child(t!("files.column.name")))
                    .child(div().w(px(110.)).child(t!("files.column.type")))
                    .child(div().w(px(100.)).child(t!("files.column.size")))
                    .child(div().w(px(150.)).child(t!("files.column.modified")))
                    .child(div().w(px(40.)).flex_none()),
            )
            .child(
                v_flex()
                    .id("files-virtual-list-wrap")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-virtual-list",
                            self.item_sizes.clone(),
                            |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|index| {
                                        let item = this.display_items.get(index)?.clone();
                                        let selected = this.selected_paths.contains(&item.path);
                                        let drag_paths =
                                            this.drag_paths_for_item(index, &item.path);
                                        let rename_input = this.renaming_input_for(&item.path);
                                        Some(Self::row(
                                            window,
                                            index,
                                            item,
                                            selected,
                                            drag_paths,
                                            rename_input,
                                            cx,
                                        ))
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    )
                    .scrollbar(&self.scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn list_view(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("files-list-view")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.clear_selection();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .child(
                h_flex()
                    .h_8()
                    .px_3()
                    .gap_3()
                    .items_center()
                    .bg(cx.theme().muted)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().w(px(28.)).flex_none())
                    .child(div().flex_1().min_w_0().child(t!("files.column.name")))
                    .child(div().w(px(40.)).flex_none()),
            )
            .child(
                v_flex()
                    .id("files-virtual-list-wrap")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-virtual-list",
                            self.item_sizes.clone(),
                            |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|index| {
                                        let item = this.display_items.get(index)?.clone();
                                        let selected = this.selected_paths.contains(&item.path);
                                        let drag_paths =
                                            this.drag_paths_for_item(index, &item.path);
                                        let rename_input = this.renaming_input_for(&item.path);
                                        Some(Self::list_row(
                                            window,
                                            index,
                                            item,
                                            selected,
                                            drag_paths,
                                            rename_input,
                                            cx,
                                        ))
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    )
                    .scrollbar(&self.scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn list_row(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let double_click_path = item.path.clone();
        let kind = item.kind;
        h_flex()
            .id(("file-list-row", index))
            .w_full()
            .h_9()
            .flex_none()
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                this.cancel_rename_if_active(cx);
                Self::dismiss_main_page_path_edit_if_active(cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground)
                    .child(Self::row_list_icon(&item, px(16.), window)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(item.display_name.clone()).into_any_element(),
                            |input| Self::inline_name_editor(input, false, cx),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn grid_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (cell_w, cell_h, icon_size) = match self.view_size_level {
            1 => (px(96.), px(72.), px(18.)),
            3 => (px(144.), px(104.), px(26.)),
            _ => (px(112.), px(80.), px(22.)),
        };

        let estimated_available_width = {
            let sidebar_w = px(240.);
            let info_pane_w = if self.show_info_pane { px(300.) } else { px(0.) };
            let padding_border = px(18.);
            (window.viewport_size().width - sidebar_w - info_pane_w - padding_border).max(px(200.))
        };
        let gap = px(8.);
        let estimated_cells_per_row =
            ((estimated_available_width + gap) / (cell_w + gap)).max(1.) as usize;
        let cells_per_row = self.grid_cells_per_row.unwrap_or(estimated_cells_per_row);
        let row_count = (self.display_items.len() + cells_per_row.saturating_sub(1)) / cells_per_row;
        let item_sizes = Rc::new(vec![size(px(1.), cell_h); row_count.max(1)]);

        v_flex()
            .id("files-grid-view")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.clear_selection();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_prepaint({
                let entity = cx.entity().clone();
                move |bounds, window, cx| {
                    let measured_width = bounds.size.width - px(18.);
                    let measured_cells =
                        ((measured_width + gap) / (cell_w + gap)).max(1.) as usize;
                    let changed = entity.update(cx, |this, _cx| {
                        let changed = this.grid_cells_per_row != Some(measured_cells);
                        this.grid_cells_per_row = Some(measured_cells);
                        changed
                    });
                    if changed {
                        window.refresh();
                    }
                }
            })
            .child(
                v_flex()
                    .id("files-grid-wrap")
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .p_2()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-grid-virtual-list",
                            item_sizes,
                            move |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|row_ix| {
                                        let start = row_ix * cells_per_row;
                                        let end = (start + cells_per_row).min(this.display_items.len());
                                        if start >= this.display_items.len() {
                                            return None;
                                        }
                                        Some(
                                            h_flex()
                                                .w_full()
                                                .gap_2()
                                                .children(
                                                    (start..end).map(|index| {
                                                        let item = this.display_items[index].clone();
                                                        let selected = this.selected_paths.contains(&item.path);
                                                        let drag_paths = this.drag_paths_for_item(index, &item.path);
                                                        let rename_input = this.renaming_input_for(&item.path);
                                                        Self::grid_cell(
                                                            window,
                                                            index,
                                                            item,
                                                            selected,
                                                            drag_paths,
                                                            rename_input,
                                                            cell_w,
                                                            cell_h,
                                                            icon_size,
                                                            cx,
                                                        )
                                                    })
                                                )
                                                .into_any_element(),
                                        )
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.grid_scroll_handle)
                        .gap_2(),
                    )
                    .scrollbar(&self.grid_scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn cards_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cell_w = px(160.);
        let cell_h = px(120.);

        let estimated_available_width = {
            let sidebar_w = px(240.);
            let info_pane_w = if self.show_info_pane { px(300.) } else { px(0.) };
            let padding_border = px(18.);
            (window.viewport_size().width - sidebar_w - info_pane_w - padding_border).max(px(200.))
        };
        let gap = px(8.);
        let estimated_cells_per_row =
            ((estimated_available_width + gap) / (cell_w + gap)).max(1.) as usize;
        let cells_per_row = self.cards_cells_per_row.unwrap_or(estimated_cells_per_row);
        let row_count = (self.display_items.len() + cells_per_row.saturating_sub(1)) / cells_per_row;
        let item_sizes = Rc::new(vec![size(px(1.), cell_h); row_count.max(1)]);

        v_flex()
            .id("files-cards-view")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.clear_selection();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_prepaint({
                let entity = cx.entity().clone();
                move |bounds, window, cx| {
                    let measured_width = bounds.size.width - px(18.);
                    let measured_cells =
                        ((measured_width + gap) / (cell_w + gap)).max(1.) as usize;
                    let changed = entity.update(cx, |this, _cx| {
                        let changed = this.cards_cells_per_row != Some(measured_cells);
                        this.cards_cells_per_row = Some(measured_cells);
                        changed
                    });
                    if changed {
                        window.refresh();
                    }
                }
            })
            .child(
                v_flex()
                    .id("files-cards-wrap")
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .p_2()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-cards-virtual-list",
                            item_sizes,
                            move |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|row_ix| {
                                        let start = row_ix * cells_per_row;
                                        let end = (start + cells_per_row).min(this.display_items.len());
                                        if start >= this.display_items.len() {
                                            return None;
                                        }
                                        Some(
                                            h_flex()
                                                .w_full()
                                                .gap_2()
                                                .children(
                                                    (start..end).map(|index| {
                                                        let item = this.display_items[index].clone();
                                                        let selected = this.selected_paths.contains(&item.path);
                                                        let drag_paths = this.drag_paths_for_item(index, &item.path);
                                                        let rename_input = this.renaming_input_for(&item.path);
                                                        Self::card_cell(
                                                            window,
                                                            index,
                                                            item,
                                                            selected,
                                                            drag_paths,
                                                            rename_input,
                                                            cx,
                                                        )
                                                    })
                                                )
                                                .into_any_element(),
                                        )
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.cards_scroll_handle)
                        .gap_2(),
                    )
                    .scrollbar(&self.cards_scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn row(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open_path = item.path.clone();
        let double_click_path = item.path.clone();
        let kind = item.kind;
        h_flex()
            .id(("file-row", index))
            .w_full()
            .h_9()
            .flex_none()
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                this.cancel_rename_if_active(cx);
                Self::dismiss_main_page_path_edit_if_active(cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground)
                    .child(Self::row_list_icon(&item, px(16.), window)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(item.display_name.clone()).into_any_element(),
                            |input| Self::inline_name_editor(input, false, cx),
                        ),
                    ),
            )
            .child(
                div()
                    .w(px(110.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(item_type_label(&item)),
            )
            .child(
                div()
                    .w(px(100.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_size(item.size)),
            )
            .child(
                div()
                    .w(px(150.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_system_time(item.modified)),
            )
            .child(
                div().w(px(40.)).flex_none().child(
                    toolbar_icon_button(format!("open-item-{index}"))
                        .icon(match kind {
                            FileItemKind::Folder => compact_icon(IconName::ChevronRight),
                            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                                compact_icon(IconName::ExternalLink)
                            }
                        })
                        .tooltip(match kind {
                            FileItemKind::Folder => t!("files.open.folder"),
                            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                                t!("files.open.file")
                            }
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_item(open_path.clone(), kind, cx);
                        })),
                ),
            )
            .into_any_element()
    }

    fn grid_cell(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        cell_w: Pixels,
        cell_h: Pixels,
        icon_size: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let name = item.display_name.clone();
        v_flex()
            .id(("file-grid-cell", index))
            .w(cell_w)
            .h(cell_h)
            .flex_none()
            .p_2()
            .gap_1()
            .items_center()
            .justify_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                this.cancel_rename_if_active(cx);
                Self::dismiss_main_page_path_edit_if_active(cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .child(Self::row_list_icon(&item, icon_size, window))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_xs()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(name).into_any_element(),
                            |input| Self::inline_name_editor(input, true, cx),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn card_cell(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let name = item.display_name.clone();
        v_flex()
            .id(("file-card-cell", index))
            .w(px(160.))
            .h(px(120.))
            .flex_none()
            .p_2()
            .gap_1()
            .items_center()
            .justify_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .child(Self::row_list_icon(&item, px(40.), window))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_sm()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(name).into_any_element(),
                            |input| Self::inline_name_editor(input, true, cx),
                        ),
                    ),
            )
            .when(item.size.is_some(), |this| {
                this.child(
                    div()
                        .w_full()
                        .text_center()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format_size(item.size)),
                )
            })
            .when(item.modified.is_some(), |this| {
                this.child(
                    div()
                        .w_full()
                        .text_center()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format_system_time(item.modified)),
                )
            })
            .into_any_element()
    }
}
