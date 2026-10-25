//! Crop overlay, handles, drag maths and aspect/orientation controls.

use eframe::egui;
use egui::{Align, Color32, Layout, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use emulsion_core::params::{CropRect, PipelineParams};

use crate::app::EmulsionApp;
use crate::crop_math;
use crate::model::{CropAspect, CropDrag, Orientation};

impl EmulsionApp {
    pub(crate) fn crop_to_screen(&self, cr: CropRect, frame: Rect) -> Rect {
        crop_math::crop_to_screen(cr, frame)
    }

    pub(crate) fn crop_center_in_stage(&self, cr: CropRect, frame: Rect) -> Pos2 {
        crop_math::crop_center_in_stage(cr, frame)
    }

    pub(crate) fn handle_crop_interaction(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        frame: Rect,
    ) {
        let Some(pos) = ui.input(|i| i.pointer.interact_pos()) else {
            return;
        };
        let primary_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        let ratio = self.constrained_frac_ratio();
        let norm = |p: Pos2| {
            (
                (p.x - frame.left()) / frame.width(),
                (p.y - frame.top()) / frame.height(),
            )
        };

        // An in-progress drag or draw.
        if self.crop_drag != CropDrag::None {
            if primary_down {
                let drag_start = self.crop_drag_start.unwrap_or(pos);
                if self.crop_drag == CropDrag::Draw {
                    if (pos - drag_start).length() > 4.0 {
                        let (ax, ay) = norm(drag_start);
                        let (mx, my) = norm(pos);
                        let crop = match ratio {
                            Some(r) => crop_math::ratio_draw((ax, ay), (mx, my), r),
                            None => crop_math::crop_from_drag(drag_start, pos, frame),
                        };
                        self.crop_rect = Some(crop);
                        self.crop_drag_rect = Some(crop);
                    }
                } else {
                    let start_rect = self.crop_drag_rect.unwrap_or(CropRect {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    });
                    let delta = pos - drag_start;
                    let nd = vec2(delta.x / frame.width(), delta.y / frame.height());
                    let crop = if self.crop_drag == CropDrag::Rect {
                        CropRect {
                            x: (start_rect.x + nd.x).clamp(0.0, 1.0 - start_rect.width),
                            y: (start_rect.y + nd.y).clamp(0.0, 1.0 - start_rect.height),
                            ..start_rect
                        }
                    } else if let Some(r) = ratio {
                        let (mx, my) = norm(pos);
                        crop_math::ratio_resize(self.crop_drag, start_rect, (mx, my), r)
                    } else {
                        crop_math::free_resize(self.crop_drag, start_rect, nd)
                    };
                    self.crop_rect = Some(crop);
                }
            } else {
                if let Some(cr) = self.crop_rect {
                    self.params.crop = Some(cr);
                    self.dirty = true;
                }
                self.crop_drag = CropDrag::None;
                self.crop_drag_start = None;
                self.crop_drag_rect = None;
            }
            return;
        }

        if !frame.contains(pos) {
            return;
        }

        // Not dragging: hovering a handle shows the resize/move cursor; a press
        // starts a move/resize on a handle, or draws a fresh rect on empty space.
        if let Some(crop) = self.crop_rect {
            let handle = self.crop_handle_at(pos, crop, frame);
            response.ctx.set_cursor_icon(match handle {
                CropDrag::TL | CropDrag::BR => egui::CursorIcon::ResizeNwSe,
                CropDrag::TR | CropDrag::BL => egui::CursorIcon::ResizeNeSw,
                CropDrag::T | CropDrag::B => egui::CursorIcon::ResizeVertical,
                CropDrag::L | CropDrag::R => egui::CursorIcon::ResizeHorizontal,
                CropDrag::Rect => egui::CursorIcon::Move,
                CropDrag::None | CropDrag::Draw => egui::CursorIcon::Default,
            });
            if primary_down && handle != CropDrag::None && !self.crop_pan {
                self.crop_drag = handle;
                self.crop_drag_start = Some(pos);
                self.crop_drag_rect = Some(crop);
            }
        } else if primary_down {
            // No crop yet: a drag draws one. Once a crop exists, dragging
            // outside it pans the image instead.
            self.begin_crop_draw(pos);
        }
    }

    fn begin_crop_draw(&mut self, pos: Pos2) {
        self.crop_drag = CropDrag::Draw;
        self.crop_drag_start = Some(pos);
        self.crop_drag_rect = None;
        self.crop_rect = None;
    }

    /// Fractional width/height ratio (in normalized 0..1 crop space) for the
    /// selected aspect + orientation, or `None` when the aspect is Free.
    fn constrained_frac_ratio(&self) -> Option<f32> {
        if self.crop_aspect.is_free() {
            return None;
        }
        let (iw, ih) = self.current_size()?;
        let pixel = self.target_crop_ratio(iw as f32, ih as f32);
        Some(pixel * (ih as f32 / iw as f32))
    }

    fn crop_handle_at(&self, pos: Pos2, crop: CropRect, frame: Rect) -> CropDrag {
        let cr = self.crop_to_screen(crop, frame);
        let handle_size = 12.0;
        let tl = Rect::from_center_size(cr.min, vec2(handle_size, handle_size));
        let tr = Rect::from_center_size(pos2(cr.max.x, cr.min.y), vec2(handle_size, handle_size));
        let bl = Rect::from_center_size(pos2(cr.min.x, cr.max.y), vec2(handle_size, handle_size));
        let br = Rect::from_center_size(cr.max, vec2(handle_size, handle_size));

        if tl.contains(pos) {
            return CropDrag::TL;
        }
        if tr.contains(pos) {
            return CropDrag::TR;
        }
        if bl.contains(pos) {
            return CropDrag::BL;
        }
        if br.contains(pos) {
            return CropDrag::BR;
        }

        // Edges
        let t = Rect::from_min_max(
            pos2(cr.min.x + handle_size, cr.min.y - 4.0),
            pos2(cr.max.x - handle_size, cr.min.y + 4.0),
        );
        let b = Rect::from_min_max(
            pos2(cr.min.x + handle_size, cr.max.y - 4.0),
            pos2(cr.max.x - handle_size, cr.max.y + 4.0),
        );
        let l = Rect::from_min_max(
            pos2(cr.min.x - 4.0, cr.min.y + handle_size),
            pos2(cr.min.x + 4.0, cr.max.y - handle_size),
        );
        let r = Rect::from_min_max(
            pos2(cr.max.x - 4.0, cr.min.y + handle_size),
            pos2(cr.max.x + 4.0, cr.max.y - handle_size),
        );

        if t.contains(pos) {
            return CropDrag::T;
        }
        if b.contains(pos) {
            return CropDrag::B;
        }
        if l.contains(pos) {
            return CropDrag::L;
        }
        if r.contains(pos) {
            return CropDrag::R;
        }

        if cr.contains(pos) {
            CropDrag::Rect
        } else {
            CropDrag::None
        }
    }

    fn target_crop_ratio(&self, iw: f32, ih: f32) -> f32 {
        let base = self.crop_aspect.ratio().unwrap_or(iw / ih);
        match self.crop_orientation {
            Orientation::Landscape => {
                if base < 1.0 {
                    1.0 / base
                } else {
                    base
                }
            }
            Orientation::Portrait => {
                if base > 1.0 {
                    1.0 / base
                } else {
                    base
                }
            }
        }
    }

    /// Resize the crop rect around its center to the selected aspect/orientation.
    fn reshape_crop(&mut self, params: &mut PipelineParams) {
        let Some((iw, ih)) = self.current_size() else {
            return;
        };
        let mut crop = self.crop_rect.unwrap_or(CropRect {
            x: 0.1,
            y: 0.1,
            width: 0.8,
            height: 0.8,
        });
        let pixel_ratio = self.target_crop_ratio(iw as f32, ih as f32);
        let frac_ratio = pixel_ratio * (ih as f32 / iw as f32);
        let cx = crop.x + crop.width * 0.5;
        let cy = crop.y + crop.height * 0.5;
        let mut w = crop.width;
        let mut h = w / frac_ratio;
        if h > 1.0 {
            h = 1.0;
            w = h * frac_ratio;
        }
        if w > 1.0 {
            w = 1.0;
            h = w / frac_ratio;
        }
        crop.width = w;
        crop.height = h;
        crop.x = (cx - w * 0.5).clamp(0.0, 1.0 - w);
        crop.y = (cy - h * 0.5).clamp(0.0, 1.0 - h);
        self.crop_rect = Some(crop);
        params.crop = Some(crop);
        self.dirty = true;
    }

    pub(crate) fn draw_crop_overlay(&self, painter: &egui::Painter, image_rect: Rect, crop: Rect) {
        let dim = Color32::from_black_alpha(195);
        let clip = painter.with_clip_rect(image_rect);
        clip.rect_filled(
            Rect::from_min_max(image_rect.min, pos2(image_rect.right(), crop.top())),
            0.0,
            dim,
        );
        clip.rect_filled(
            Rect::from_min_max(pos2(image_rect.left(), crop.bottom()), image_rect.max),
            0.0,
            dim,
        );
        clip.rect_filled(
            Rect::from_min_max(
                pos2(image_rect.left(), crop.top()),
                pos2(crop.left(), crop.bottom()),
            ),
            0.0,
            dim,
        );
        clip.rect_filled(
            Rect::from_min_max(
                pos2(crop.right(), crop.top()),
                pos2(image_rect.right(), crop.bottom()),
            ),
            0.0,
            dim,
        );
    }

    pub(crate) fn draw_crop_frame(&self, painter: &egui::Painter, crop: Rect) {
        painter.rect_stroke(
            crop,
            0.0,
            Stroke::new(1.5, Color32::WHITE),
            StrokeKind::Inside,
        );
        let guide = Color32::from_white_alpha(34);
        for i in 1..3 {
            let t = crop.width() * i as f32 / 3.0;
            let x = crop.left() + t;
            painter.line_segment(
                [pos2(x, crop.top()), pos2(x, crop.bottom())],
                Stroke::new(1.0, guide),
            );
            let y = crop.top() + crop.height() * i as f32 / 3.0;
            painter.line_segment(
                [pos2(crop.left(), y), pos2(crop.right(), y)],
                Stroke::new(1.0, guide),
            );
        }
    }

    pub(crate) fn crop_controls(&mut self, ui: &mut egui::Ui, params: &mut PipelineParams) {
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 50.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                let mut aspect = self.crop_aspect;
                egui::ComboBox::from_id_salt("crop_aspect")
                    .selected_text(aspect.label())
                    .width(92.0)
                    .show_ui(ui, |ui| {
                        for option in CropAspect::ALL {
                            ui.selectable_value(&mut aspect, option, option.label());
                        }
                    });
                if aspect != self.crop_aspect {
                    self.crop_aspect = aspect;
                    if !aspect.is_free() {
                        self.reshape_crop(params);
                    }
                }

                ui.add_space(12.0);
                let mut orientation = self.crop_orientation;
                egui::ComboBox::from_id_salt("crop_orientation")
                    .selected_text(match orientation {
                        Orientation::Portrait => "Portrait",
                        Orientation::Landscape => "Landscape",
                    })
                    .width(104.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut orientation, Orientation::Portrait, "Portrait");
                        ui.selectable_value(&mut orientation, Orientation::Landscape, "Landscape");
                    });
                if orientation != self.crop_orientation {
                    self.crop_orientation = orientation;
                    if !self.crop_aspect.is_free() {
                        self.reshape_crop(params);
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if crate::util::reset_icon_button(ui, "Reset crop") {
                        params.crop = None;
                        self.crop_rect = None;
                        self.crop_drag = CropDrag::None;
                        self.crop_drag_start = None;
                        self.crop_drag_rect = None;
                    }
                });
            },
        );
    }
}
