//! Tool strip and the expanded adjustment panel.

use eframe::egui;
use egui::scroll_area::ScrollBarVisibility;
use egui::{
    Align, Align2, Color32, FontId, Layout, Margin, RichText, Sense, Stroke, StrokeKind, pos2, vec2,
};
use emulsion_core::film;
use emulsion_core::params::PipelineParams;

use crate::app::EmulsionApp;
use crate::model::{CropDrag, Orientation, Tool};
use crate::theme::{ACCENT, ACCENT_LINE, ACCENT_SOFT, DIM, HOVER, LINE, MUTED, PANEL_TOP, TEXT};
use crate::crop_math::normalize_crop;

impl EmulsionApp {
    pub(crate) fn settings_panel(&mut self, ui: &mut egui::Ui) {
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Slash)) {
            if self.open_tool == Some(Tool::Simulations) {
                self.film_focus_search = true;
            } else {
                self.focus_search = true;
            }
        }

        let query = self.tool_search.trim().to_lowercase();
        let invert_before = self.params.invert;
        let film_before = self.params.film;
        let mut params = self.params;
        let mut open_tool = self.open_tool;
        let mut search_id = None;

        let frame = egui::Frame::new()
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(8.0)
            .inner_margin(Margin::symmetric(10, 5));
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add_sized(
                    vec2(142.0, 30.0),
                    egui::TextEdit::singleline(&mut self.tool_search)
                        .hint_text(RichText::new("Search tools").color(DIM))
                        .font(FontId::proportional(16.0))
                        .text_color(TEXT)
                        .align(Align2::LEFT_CENTER)
                        .margin(Margin::ZERO)
                        .frame(egui::Frame::NONE),
                );
                search_id = Some(response.id);
                if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.tool_search.clear();
                    response.surrender_focus();
                }

                let (rect, _) = ui.allocate_exact_size(vec2(1.0, 18.0), Sense::hover());
                ui.painter().rect_filled(rect, 0.0, LINE);

                let available_width = ui.available_width();
                egui::ScrollArea::horizontal()
                    .id_salt("tools_horizontal_scroll")
                    .max_width(available_width)
                    .auto_shrink([false, true])
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            const TOOLS: &[Tool] = &[
                                Tool::Film,
                                Tool::Simulations,
                                Tool::Crop,
                                Tool::Rotate,
                                Tool::Ev,
                                Tool::Contrast,
                                Tool::Shadows,
                                Tool::Highlights,
                                Tool::Wb,
                                Tool::Tint,
                                Tool::Saturation,
                                Tool::Vignette,
                            ];
                            for &tool in TOOLS {
                                if tool_visible(&query, tool) {
                                    let response = tool_tab(
                                        ui,
                                        tool.label(),
                                        open_tool == Some(tool),
                                        tool.changed(&params),
                                    );
                                    if response.clicked() {
                                        open_tool = toggle_tool(open_tool, tool);
                                    }
                                }
                            }
                        })
                    });
            });
        });

        if let Some(id) = search_id
            && self.focus_search
        {
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
            self.focus_search = false;
        }

        if let Some(tool) = open_tool {
            let frame = egui::Frame::new()
                .fill(PANEL_TOP)
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(10.0)
                .inner_margin(Margin::symmetric(15, 0));
            frame.show(ui, |ui| match tool {
                Tool::Simulations => self.film_controls(ui, &mut params),
                Tool::Crop => self.crop_controls(ui, &mut params),
                _ => tool_body(ui, tool, &mut params),
            });
        }

        if params != self.params {
            self.params = params;
            self.dirty = true;
            if params.film != film_before {
                match params.film.and_then(film::name) {
                    Some(name) => self.show_toast(format!("Simulations · {}", name)),
                    None => self.show_toast("Simulations · none"),
                }
            } else if params.invert != invert_before {
                self.show_toast(if params.invert {
                    "Develop · inverted"
                } else {
                    "Develop · positive"
                });
            }
        }
        if open_tool != self.open_tool {
            if open_tool == Some(Tool::Crop) {
                // Enter crop: draft starts from the applied crop (may be none).
                self.crop_rect = self.params.crop;
                self.crop_drag = CropDrag::None;
                self.crop_drag_start = None;
                self.crop_drag_rect = None;
                self.crop_pan = false;
                if let Some((w, h)) = self.current_size() {
                    self.crop_orientation = if w >= h {
                        Orientation::Landscape
                    } else {
                        Orientation::Portrait
                    };
                }
                self.dirty = true;
            } else if self.open_tool == Some(Tool::Crop) {
                // Leave crop: apply the draft. A tiny or full-frame rect is "no crop".
                let previous = self.params.crop;
                let applied = self.crop_rect.and_then(normalize_crop);
                self.params.crop = applied;
                self.crop_rect = applied;
                self.crop_drag = CropDrag::None;
                self.crop_drag_start = None;
                self.crop_drag_rect = None;
                self.crop_pan = false;
                self.dirty = true;
                if applied != previous {
                    self.show_toast(if applied.is_some() {
                        "Crop applied"
                    } else {
                        "Crop removed"
                    });
                }
            }
            self.open_tool = open_tool;
        }
    }
}

fn tool_body(ui: &mut egui::Ui, tool: Tool, params: &mut PipelineParams) {
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), 50.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.allocate_ui_with_layout(
                vec2(140.0, 50.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                },
            );

            if tool == Tool::Rotate {
                rotate_buttons(ui, params);
            }

            if tool == Tool::Film {
                develop_buttons(ui, params);
            }

            // Standard slider + dragvalue + reset for the remaining tools.
            let Some(value) = tool.value_mut(params) else {
                return;
            };
            ui.spacing_mut().slider_width = ui.available_width() - 99.0;
            let response = ui.add(
                egui::Slider::new(value, tool.range())
                    .show_value(false)
                    .trailing_fill(true)
                    .step_by(if tool.decimals() > 0 { 0.1 } else { 1.0 }),
            );
            let reset = ui.input(|i| {
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary)
                    && i.pointer
                        .interact_pos()
                        .is_some_and(|pos| response.rect.contains(pos))
            });
            if reset {
                *value = 0.0;
            }

            let decimals = tool.decimals();
            let suffix = tool.suffix();
            let mut shown = *value;
            ui.allocate_ui_with_layout(
                vec2(56.0, 32.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.visuals_mut().override_text_color = Some(ACCENT);
                    let response = ui.add(
                        egui::DragValue::new(&mut shown)
                            .range(tool.range())
                            .speed(if decimals > 0 { 0.1 } else { 1.0 })
                            .fixed_decimals(decimals)
                            .custom_formatter(move |value, _| {
                                signed_value(value, decimals, suffix)
                            }),
                    );
                    if response.changed() {
                        *value = shown;
                    }
                },
            );

            let reset_clicked = ui
                .add(
                    egui::Button::new(RichText::new("↺").size(15.0).color(DIM))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .corner_radius(6.0)
                        .min_size(vec2(28.0, 28.0)),
                )
                .on_hover_text("Reset adjustment")
                .clicked();
            if reset_clicked {
                *value = 0.0;
                if tool == Tool::Rotate {
                    params.rotate_90 = 0;
                    params.flip_horizontal = false;
                }
            }
        },
    );
}

fn rotate_buttons(ui: &mut egui::Ui, params: &mut PipelineParams) {
    let btn = |label: &str| {
        egui::Button::new(RichText::new(label).size(12.0).color(MUTED))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(6.0)
            .min_size(vec2(52.0, 28.0))
    };
    if ui.add(btn("90L")).clicked() {
        params.rotate_90 = (params.rotate_90 + 3) % 4;
    }
    if ui.add(btn("90R")).clicked() {
        params.rotate_90 = (params.rotate_90 + 1) % 4;
    }
    let flip = egui::Button::new(RichText::new("FLIP").size(12.0).color(
        if params.flip_horizontal {
            ACCENT
        } else {
            MUTED
        },
    ))
    .fill(if params.flip_horizontal {
        ACCENT_SOFT
    } else {
        Color32::TRANSPARENT
    })
    .stroke(Stroke::new(
        1.0,
        if params.flip_horizontal {
            ACCENT_LINE
        } else {
            LINE
        },
    ))
    .corner_radius(6.0)
    .min_size(vec2(52.0, 28.0));
    if ui.add(flip).clicked() {
        params.flip_horizontal = !params.flip_horizontal;
    }
    ui.add_space(4.0);
}

fn develop_buttons(ui: &mut egui::Ui, params: &mut PipelineParams) {
    let btn = |label: &str| {
        egui::Button::new(RichText::new(label).size(12.0).color(MUTED))
            .fill(if params.invert {
                ACCENT_SOFT
            } else {
                Color32::TRANSPARENT
            })
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(6.0)
            .min_size(vec2(52.0, 28.0))
    };
    if ui.add(btn("DEVELOP")).clicked() {
        params.invert = !params.invert;
    }
    ui.add_space(4.0);
}

fn tool_tab(ui: &mut egui::Ui, label: &str, active: bool, changed: bool) -> egui::Response {
    let font = FontId::proportional(12.0);
    let text_width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), MUTED)
        .size()
        .x;
    let (rect, response) = ui.allocate_exact_size(vec2(text_width + 26.0, 32.0), Sense::click());
    let hovered = response.hovered();
    let color = if active {
        ACCENT
    } else if hovered {
        TEXT
    } else if changed {
        Color32::from_rgb(77, 67, 50)
    } else {
        MUTED
    };
    let fill = if active {
        ACCENT_SOFT
    } else if hovered {
        HOVER
    } else {
        Color32::TRANSPARENT
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 7.0, fill);
    if active {
        painter.rect_stroke(rect, 7.0, Stroke::new(1.0, ACCENT_LINE), StrokeKind::Inside);
    }
    painter.text(rect.center(), Align2::CENTER_CENTER, label, font, color);
    if changed && !active {
        painter.circle_filled(pos2(rect.right() - 7.0, rect.top() + 8.0), 2.0, ACCENT);
    }
    response
}

fn tool_visible(query: &str, tool: Tool) -> bool {
    query.is_empty()
        || tool.label().to_lowercase().contains(query)
        || tool.name().to_lowercase().contains(query)
        || tool.aliases().contains(query)
}

fn toggle_tool(open: Option<Tool>, tool: Tool) -> Option<Tool> {
    if open == Some(tool) { None } else { Some(tool) }
}

fn signed_value(value: f64, decimals: usize, suffix: &str) -> String {
    let sign = if value > 0.0 { "+" } else { "" };
    format!("{}{:.*}{}", sign, decimals, value, suffix)
}
