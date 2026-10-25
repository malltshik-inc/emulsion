//! Canvas stage: image painting, zoom/pan and the stage background.

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::app::EmulsionApp;
use crate::model::{CropDrag, Tool};
use crate::theme::{ACCENT, DIM, LINE, LINE_STRONG, MUTED, STAGE_IN, STAGE_OUT};

impl EmulsionApp {
    pub(crate) fn stage(&mut self, ui: &mut egui::Ui) {
        if ui.input(|i| i.key_pressed(egui::Key::Space)) && !ui.ctx().egui_wants_keyboard_input() {
            self.pan = egui::Vec2::ZERO;
        }

        let (response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::click_and_drag());
        let rect = response.rect;
        paint_stage_background(&painter, rect);

        if self.texture.is_none() {
            if self.files.is_empty() {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "Open a folder to start",
                    FontId::proportional(18.0),
                    MUTED,
                );
                if response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.open_folder();
                }
            } else if self.cursor.is_none() {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "Select file to start",
                    FontId::proportional(18.0),
                    MUTED,
                );
            } else if self.loading {
                paint_loading_line(&painter, rect.center(), ui.input(|i| i.time) as f32);
                ui.ctx().request_repaint();
            }
            return;
        }

        // Get size first (Copy, no long-lived borrow)
        let size = match self.texture_size {
            Some(s) => s,
            None => return,
        };

        // Crop interaction (needs mutable self)
        let img_w = size.x;
        let img_h = size.y;
        let content = Rect::from_min_max(rect.min + vec2(28.0, 26.0), rect.max - vec2(28.0, 46.0));
        if content.width() <= 0.0 || content.height() <= 0.0 {
            return;
        }
        let fit = (content.width() / img_w).min(content.height() / img_h);
        if self.zoom <= 0.0 {
            self.zoom = fit;
            self.pan = egui::Vec2::ZERO;
        } else if (self.zoom - self.fit_zoom).abs() < 0.002
            && (self.last_stage - content.size()).length() > 0.5
        {
            self.zoom = fit;
        }
        self.fit_zoom = fit;
        self.last_stage = content.size();

        let w = img_w * self.zoom;
        let h = img_h * self.zoom;
        let frame_rect =
            Rect::from_center_size(rect.center() + self.pan + vec2(0.0, -8.0), vec2(w, h));

        // Crop rect in screen coordinates
        let cropping = self.open_tool == Some(Tool::Crop);
        let crop_screen = self.crop_rect.map(|cr| self.crop_to_screen(cr, frame_rect));

        // Handle crop interaction (needs mutable self)
        if cropping {
            self.handle_crop_interaction(ui, &response, frame_rect);
        }

        // Space recenter on crop center if crop exists
        if ui.input(|i| i.key_pressed(egui::Key::Space)) && !ui.ctx().egui_wants_keyboard_input() {
            if let Some(cr) = self.crop_rect {
                let crop_center = self.crop_center_in_stage(cr, frame_rect);
                self.pan = rect.center() - crop_center;
            } else {
                self.pan = egui::Vec2::ZERO;
            }
        }

        // Now get texture for drawing (after mutable borrows are done)
        let texture = match self.texture.as_ref() {
            Some(t) => t,
            None => return,
        };

        // Draw image
        let shadow = egui::epaint::Shadow {
            offset: [0, 22],
            blur: 54,
            spread: 0,
            color: Color32::from_black_alpha(44),
        };
        painter.add(shadow.as_shape(frame_rect, 5.0));
        painter.rect_filled(frame_rect, 5.0, Color32::from_rgb(233, 225, 209));
        egui::Image::from_texture(texture)
            .corner_radius(5.0)
            .maintain_aspect_ratio(false)
            .fit_to_exact_size(frame_rect.size())
            .paint_at(ui, frame_rect);

        // Draw crop overlay (darken outside) and crop frame while cropping
        if cropping && let Some(cr) = crop_screen {
            self.draw_crop_overlay(&painter, frame_rect, cr);
            self.draw_crop_frame(&painter, cr);
        }

        painter.rect_stroke(
            frame_rect,
            5.0,
            Stroke::new(1.0, LINE_STRONG),
            StrokeKind::Inside,
        );

        let info = format!("{}  ·  Space recentres", self.status);
        let info_painter = painter.with_clip_rect(Rect::from_min_max(
            pos2(rect.left() + 20.0, rect.bottom() - 34.0),
            pos2(rect.center().x + 120.0, rect.bottom() - 8.0),
        ));
        info_painter.text(
            pos2(rect.left() + 22.0, rect.bottom() - 18.0),
            Align2::LEFT_CENTER,
            info,
            FontId::monospace(11.5),
            DIM,
        );
        let right = self
            .current_size()
            .map(|(w, h)| format!("sRGB · {} × {}", w, h))
            .unwrap_or_else(|| String::from("sRGB"));
        painter.text(
            pos2(rect.right() - 22.0, rect.bottom() - 18.0),
            Align2::RIGHT_CENTER,
            right,
            FontId::monospace(11.5),
            DIM,
        );

        let scroll = ui.input(|i| i.smooth_scroll_delta().y);
        if response.hovered() && scroll != 0.0 {
            self.zoom = (self.zoom * (1.0 + scroll * 0.005)).clamp(0.01, 40.0);
            ui.ctx().request_repaint();
        }
        // Dragging inside the crop edits it; a drag anywhere else pans the image.
        let primary_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        if !primary_down {
            self.crop_pan = false;
        }
        if self.crop_drag == CropDrag::None && response.is_pointer_button_down_on() {
            let delta = ui.input(|i| i.pointer.delta());
            self.pan += delta;
            // While cropping, keep the crop rect fixed on screen: panning the
            // image slides it underneath, so the crop's image coords shift back.
            if cropping && let Some(cr) = self.crop_rect {
                self.crop_pan = true;
                if delta != egui::Vec2::ZERO {
                    let next = emulsion_core::params::CropRect {
                        x: (cr.x - delta.x / frame_rect.width()).clamp(0.0, 1.0 - cr.width),
                        y: (cr.y - delta.y / frame_rect.height()).clamp(0.0, 1.0 - cr.height),
                        ..cr
                    };
                    self.crop_rect = Some(next);
                    self.params.crop = Some(next);
                }
            }
        }
    }
}

fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
    Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

fn paint_stage_background(painter: &egui::Painter, rect: Rect) {
    let columns = 16;
    let rows = 12;
    let mut mesh = egui::epaint::Mesh::default();
    for row in 0..=rows {
        for column in 0..=columns {
            let u = column as f32 / columns as f32;
            let v = row as f32 / rows as f32;
            let pos = pos2(
                rect.left() + rect.width() * u,
                rect.top() + rect.height() * v,
            );
            let dx = (u - 0.5) * 1.15;
            let dy = (v - 0.42) * 1.0;
            let distance = (dx * dx + dy * dy).sqrt() / 0.78;
            let t = distance.clamp(0.0, 1.0);
            let smooth = t * t * (3.0 - 2.0 * t);
            mesh.colored_vertex(pos, lerp_color(STAGE_IN, STAGE_OUT, smooth));
        }
    }
    for row in 0..rows {
        for column in 0..columns {
            let i = row * (columns + 1) + column;
            mesh.add_triangle(i, i + 1, i + columns + 2);
            mesh.add_triangle(i, i + columns + 2, i + columns + 1);
        }
    }
    painter.add(egui::Shape::mesh(mesh));

    let mut y = rect.top() + 13.0;
    while y < rect.bottom() {
        let mut x = rect.left() + 13.0;
        while x < rect.right() {
            let u = (x - rect.left()) / rect.width();
            let v = (y - rect.top()) / rect.height();
            let dx = (u - 0.5) * 1.15;
            let dy = (v - 0.42) * 1.0;
            let distance = (dx * dx + dy * dy).sqrt() / 0.78;
            let alpha = ((1.0 - distance.clamp(0.0, 1.0)).powf(1.4) * 34.0) as u8;
            if alpha > 1 {
                painter.circle_filled(
                    pos2(x, y),
                    0.8,
                    Color32::from_rgba_unmultiplied(56, 44, 28, alpha),
                );
            }
            x += 26.0;
        }
        y += 26.0;
    }
}

fn paint_loading_line(painter: &egui::Painter, center: Pos2, time: f32) {
    let width = 150.0;
    let track = Rect::from_center_size(center, vec2(width, 2.0));
    painter.rect_filled(track, 1.0, LINE);
    let segment = width * 0.32;
    let phase = (time * 0.7).fract();
    let start = track.left() + phase * (width + segment) - segment;
    let clipped = painter.with_clip_rect(track);
    clipped.rect_filled(
        Rect::from_min_size(pos2(start, track.top()), vec2(segment, track.height())),
        1.0,
        ACCENT,
    );
}
