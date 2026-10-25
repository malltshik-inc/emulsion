//! Top bar: branding, current file metadata and edit/export actions.

use eframe::egui;
use egui::{Align2, Color32, FontFamily, FontId, Rect, RichText, Stroke, StrokeKind, pos2, vec2};

use crate::app::EmulsionApp;
use crate::theme::{ACCENT, ACCENT_SOFT, DIM, LINE, LINE_STRONG, MUTED, TEXT};

impl EmulsionApp {
    pub(crate) fn top_bar(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let painter = ui.painter().with_clip_rect(rect);
        let center_y = rect.center().y;

        let logo_rect = Rect::from_min_size(pos2(rect.left(), center_y - 18.0), vec2(36.0, 36.0));
        painter.rect_filled(
            logo_rect.translate(vec2(0.0, 3.0)),
            9.0,
            Color32::from_black_alpha(24),
        );
        if let Some(logo) = &self.logo {
            egui::Image::from_texture(logo)
                .fit_to_exact_size(logo_rect.size())
                .corner_radius(9.0)
                .paint_at(ui, logo_rect);
        } else {
            painter.rect_filled(logo_rect, 9.0, ACCENT_SOFT);
            painter.text(
                logo_rect.center(),
                Align2::CENTER_CENTER,
                "E",
                FontId::new(20.0, self.wordmark_family.clone()),
                ACCENT,
            );
        }
        painter.rect_stroke(
            logo_rect,
            9.0,
            Stroke::new(1.0, LINE_STRONG),
            StrokeKind::Inside,
        );

        let word_font = FontId::new(20.0, self.wordmark_family.clone());
        painter.text(
            pos2(logo_rect.right() + 10.0, center_y),
            Align2::LEFT_CENTER,
            "Emulsion",
            word_font.clone(),
            TEXT,
        );

        let has_file = self.cursor.is_some();
        if has_file {
            let name_font = FontId::new(14.5, FontFamily::Proportional);
            let meta_font = FontId::monospace(11.5);
            let name = self.current_name();
            let meta = self.current_meta();
            let name_galley = painter.layout_no_wrap(name, name_font, TEXT);
            let meta_galley = painter.layout_no_wrap(meta, meta_font, DIM);
            let edited = self.is_current_edited();
            let dot_space = if edited { 15.0 } else { 0.0 };
            let meta_pad = 16.0;
            let total = dot_space + name_galley.size().x + 9.0 + meta_galley.size().x + meta_pad;
            let mut x = rect.center().x - total / 2.0;
            if edited {
                painter.circle_filled(pos2(x + 3.0, center_y), 3.0, ACCENT);
                x += dot_space;
            }
            painter.galley(
                pos2(x, center_y - name_galley.size().y / 2.0),
                name_galley.clone(),
                TEXT,
            );
            x += name_galley.size().x + 9.0;
            let meta_rect = Rect::from_min_size(
                pos2(x, center_y - 11.0),
                vec2(meta_galley.size().x + meta_pad, 22.0),
            );
            painter.rect_filled(
                meta_rect,
                5.0,
                Color32::from_rgba_unmultiplied(255, 252, 245, 150),
            );
            painter.rect_stroke(meta_rect, 5.0, Stroke::new(1.0, LINE), StrokeKind::Inside);
            painter.galley(
                pos2(
                    meta_rect.left() + meta_pad / 2.0,
                    meta_rect.center().y - meta_galley.size().y / 2.0,
                ),
                meta_galley,
                DIM,
            );

            let export_rect = Rect::from_min_size(
                pos2(rect.right() - 16.0 - 98.0, center_y - 16.0),
                vec2(98.0, 32.0),
            );
            let reset_rect = Rect::from_min_size(
                pos2(export_rect.left() - 8.0 - 66.0, center_y - 16.0),
                vec2(66.0, 32.0),
            );

            let reset = ui.put(
                reset_rect,
                egui::Button::new(RichText::new("Reset").size(14.0).color(MUTED))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(8.0)
                    .min_size(reset_rect.size()),
            );
            if reset.clicked() {
                self.reset_all();
            }

            painter.rect_filled(
                export_rect.translate(vec2(0.0, 4.0)),
                8.0,
                Color32::from_rgba_unmultiplied(135, 63, 29, 48),
            );
            let export = ui.put(
                export_rect,
                egui::Button::new(
                    RichText::new("Export")
                        .size(14.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(ACCENT)
                .stroke(Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(135, 63, 29, 92),
                ))
                .corner_radius(8.0)
                .min_size(export_rect.size()),
            );
            if export.clicked() {
                self.export_dialog = true;
            }
        }
    }
}
