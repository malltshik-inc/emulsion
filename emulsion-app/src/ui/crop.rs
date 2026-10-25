//! Crop overlay, handles, drag maths and aspect/orientation controls.

use eframe::egui;
use egui::{Align, Color32, Layout, Pos2, Rect, RichText, Stroke, StrokeKind, Vec2, pos2, vec2};
use emulsion_core::params::{CropRect, PipelineParams};

use crate::app::EmulsionApp;
use crate::model::{CropAspect, CropDrag, Orientation};
use crate::theme::DIM;

impl EmulsionApp {
    pub(crate) fn crop_to_screen(&self, cr: CropRect, frame: Rect) -> Rect {
        Rect::from_min_max(
            pos2(
                frame.left() + cr.x * frame.width(),
                frame.top() + cr.y * frame.height(),
            ),
            pos2(
                frame.left() + (cr.x + cr.width) * frame.width(),
                frame.top() + (cr.y + cr.height) * frame.height(),
            ),
        )
    }

    pub(crate) fn crop_center_in_stage(&self, cr: CropRect, frame: Rect) -> Pos2 {
        pos2(
            frame.left() + (cr.x + cr.width * 0.5) * frame.width(),
            frame.top() + (cr.y + cr.height * 0.5) * frame.height(),
        )
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
                            Some(r) => ratio_draw((ax, ay), (mx, my), r),
                            None => self.crop_from_drag(drag_start, pos, frame),
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
                        ratio_resize(self.crop_drag, start_rect, (mx, my), r)
                    } else {
                        free_resize(self.crop_drag, start_rect, nd)
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

    fn crop_from_drag(&self, a: Pos2, b: Pos2, frame: Rect) -> CropRect {
        let min = a.min(b);
        let max = a.max(b);
        let x = ((min.x - frame.left()) / frame.width()).clamp(0.0, 1.0);
        let y = ((min.y - frame.top()) / frame.height()).clamp(0.0, 1.0);
        let right = ((max.x - frame.left()) / frame.width()).clamp(0.0, 1.0);
        let bottom = ((max.y - frame.top()) / frame.height()).clamp(0.0, 1.0);
        CropRect {
            x,
            y,
            width: (right - x).max(0.0),
            height: (bottom - y).max(0.0),
        }
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
                    let reset = ui
                        .add(
                            egui::Button::new(RichText::new("\u{21ba}").size(15.0).color(DIM))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .corner_radius(6.0)
                                .min_size(vec2(28.0, 28.0)),
                        )
                        .on_hover_text("Reset crop");
                    if reset.clicked() {
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

pub(crate) fn normalize_crop(cr: CropRect) -> Option<CropRect> {
    if cr.width < 0.02 || cr.height < 0.02 {
        return None;
    }
    if cr.x <= 0.001 && cr.y <= 0.001 && cr.width >= 0.999 && cr.height >= 0.999 {
        return None;
    }
    Some(cr)
}

fn free_resize(handle: CropDrag, start: CropRect, nd: Vec2) -> CropRect {
    let mut crop = start;
    match handle {
        CropDrag::Rect => {
            crop.x = (start.x + nd.x).clamp(0.0, 1.0 - start.width);
            crop.y = (start.y + nd.y).clamp(0.0, 1.0 - start.height);
        }
        CropDrag::TL => {
            crop.x = (start.x + nd.x).clamp(0.0, start.x + start.width - 0.01);
            crop.y = (start.y + nd.y).clamp(0.0, start.y + start.height - 0.01);
            crop.width = (start.width - nd.x).max(0.01);
            crop.height = (start.height - nd.y).max(0.01);
        }
        CropDrag::TR => {
            crop.y = (start.y + nd.y).clamp(0.0, start.y + start.height - 0.01);
            crop.width = (start.width + nd.x).max(0.01).min(1.0 - start.x);
            crop.height = (start.height - nd.y).max(0.01);
        }
        CropDrag::BL => {
            crop.x = (start.x + nd.x).clamp(0.0, start.x + start.width - 0.01);
            crop.width = (start.width - nd.x).max(0.01);
            crop.height = (start.height + nd.y).max(0.01).min(1.0 - start.y);
        }
        CropDrag::BR => {
            crop.width = (start.width + nd.x).max(0.01).min(1.0 - start.x);
            crop.height = (start.height + nd.y).max(0.01).min(1.0 - start.y);
        }
        CropDrag::T => {
            crop.y = (start.y + nd.y).clamp(0.0, start.y + start.height - 0.01);
            crop.height = (start.height - nd.y).max(0.01);
        }
        CropDrag::B => {
            crop.height = (start.height + nd.y).max(0.01).min(1.0 - start.y);
        }
        CropDrag::L => {
            crop.x = (start.x + nd.x).clamp(0.0, start.x + start.width - 0.01);
            crop.width = (start.width - nd.x).max(0.01);
        }
        CropDrag::R => {
            crop.width = (start.width + nd.x).max(0.01).min(1.0 - start.x);
        }
        CropDrag::None | CropDrag::Draw => {}
    }
    crop
}

/// Resize a handle while keeping width/height = `r`, anchored so the opposite
/// corner (or edge) stays put.
fn ratio_resize(handle: CropDrag, start: CropRect, mouse: (f32, f32), r: f32) -> CropRect {
    let (mx, my) = mouse;
    let cx = start.x + start.width * 0.5;
    let cy = start.y + start.height * 0.5;
    let (x, y, mut w, mut h) = match handle {
        CropDrag::BR => {
            let w = (mx - start.x).max(0.0);
            (start.x, start.y, w, w / r)
        }
        CropDrag::TL => {
            let ax = start.x + start.width;
            let ay = start.y + start.height;
            let w = (ax - mx).max(0.0);
            (ax - w, ay - w / r, w, w / r)
        }
        CropDrag::TR => {
            let ax = start.x;
            let ay = start.y + start.height;
            let w = (mx - ax).max(0.0);
            (ax, ay - w / r, w, w / r)
        }
        CropDrag::BL => {
            let ax = start.x + start.width;
            let ay = start.y;
            let w = (ax - mx).max(0.0);
            (ax - w, ay, w, w / r)
        }
        CropDrag::T => {
            let ay = start.y + start.height;
            let h = (ay - my).max(0.0);
            (cx - h * r * 0.5, ay - h, h * r, h)
        }
        CropDrag::B => {
            let ay = start.y;
            let h = (my - ay).max(0.0);
            (cx - h * r * 0.5, ay, h * r, h)
        }
        CropDrag::L => {
            let ax = start.x + start.width;
            let w = (ax - mx).max(0.0);
            (ax - w, cy - w / r * 0.5, w, w / r)
        }
        CropDrag::R => {
            let ax = start.x;
            let w = (mx - ax).max(0.0);
            (ax, cy - w / r * 0.5, w, w / r)
        }
        CropDrag::None | CropDrag::Rect | CropDrag::Draw => {
            (start.x, start.y, start.width, start.height)
        }
    };
    if w > 1.0 {
        w = 1.0;
        h = w / r;
    }
    if h > 1.0 {
        h = 1.0;
        w = h * r;
    }
    CropRect {
        x: x.clamp(0.0, (1.0 - w).max(0.0)),
        y: y.clamp(0.0, (1.0 - h).max(0.0)),
        width: w,
        height: h,
    }
}

/// Draw a fresh rect from `anchor` toward `mouse`, keeping width/height = `r`
/// and staying inside the unit square.
fn ratio_draw(anchor: (f32, f32), mouse: (f32, f32), r: f32) -> CropRect {
    let (ax, ay) = anchor;
    let (mx, my) = mouse;
    let sx = if mx < ax { -1.0 } else { 1.0 };
    let sy = if my < ay { -1.0 } else { 1.0 };
    let mut w = (mx - ax).abs();
    let mut h = (my - ay).abs();
    if w / r >= h {
        h = w / r;
    } else {
        w = h * r;
    }
    let max_w = if sx > 0.0 { 1.0 - ax } else { ax };
    let max_h = if sy > 0.0 { 1.0 - ay } else { ay };
    if w > max_w {
        w = max_w.max(0.0);
        h = w / r;
    }
    if h > max_h {
        h = max_h.max(0.0);
        w = h * r;
    }
    let x = if sx > 0.0 { ax } else { ax - w };
    let y = if sy > 0.0 { ay } else { ay - h };
    CropRect {
        x: x.clamp(0.0, 1.0),
        y: y.clamp(0.0, 1.0),
        width: w,
        height: h,
    }
}
