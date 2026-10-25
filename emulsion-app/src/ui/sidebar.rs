//! Library sidebar: file list, thumbnails and histogram/stats footer.

use eframe::egui;
use egui::{
    Align, Align2, Color32, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, pos2, vec2,
};

use crate::app::EmulsionApp;
use crate::histogram::paint_histogram;
use crate::theme::{ACCENT, ACCENT_LINE, ACCENT_SOFT, DIM, HOVER, LINE, MUTED, TEXT};
use crate::util::{aspect_label, cover_uv, file_extension, file_name};

impl EmulsionApp {
    pub(crate) fn sidebar(&mut self, ui: &mut egui::Ui) {
        let mut open_clicked = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("LIBRARY").size(11.5).strong().color(DIM));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("Open").size(12.0).color(MUTED))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0, LINE))
                            .corner_radius(6.0)
                            .min_size(vec2(50.0, 24.0)),
                    )
                    .on_hover_text("Open folder…")
                    .clicked()
                {
                    open_clicked = true;
                }
            });
        });

        let available = ui.available_height();
        let hist_height = if available > 560.0 { 120.0 } else { 82.0 };

        egui::Panel::bottom("sidebar_preview")
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                ui.add_space(6.0);
                let (sep_rect, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(sep_rect, 0.0, LINE);
                ui.add_space(11.0);

                let dims = self
                    .current_size()
                    .map(|(w, h)| format!("{} × {}", w, h))
                    .unwrap_or_else(|| String::from("—"));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("HISTOGRAM").size(11.5).strong().color(DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(dims).size(11.5).monospace().color(DIM));
                    });
                });
                ui.add_space(6.0);
                paint_histogram(ui, self.histogram.as_ref(), hist_height);
                ui.add_space(10.0);

                let (mp, aspect) = match self.current_size() {
                    Some((w, h)) => (
                        format!("{:.1}", w as f64 * h as f64 / 1_000_000.0),
                        aspect_label(w, h),
                    ),
                    None => (String::from("—"), String::from("—")),
                };
                ui.columns(3, |cols| {
                    stat_box(&mut cols[0], "MP", mp.as_str());
                    stat_box(&mut cols[1], "RATIO", aspect.as_str());
                    stat_box(&mut cols[2], "PROFILE", "sRGB");
                });
            });

        let list_height = ui.available_height().max(24.0);
        let (list_rect, list_response) =
            ui.allocate_exact_size(vec2(ui.available_width(), list_height), Sense::click());
        if self.files.is_empty() {
            if list_response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                open_clicked = true;
            }
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(list_rect)
                    .layout(Layout::top_down(Align::Center)),
                |ui| {
                    ui.add_space(28.0);
                    ui.label(RichText::new("No images yet").size(13.0).color(MUTED));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Open a folder").size(11.5).color(DIM));
                },
            );
        } else {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(list_rect)
                    .layout(Layout::top_down(Align::Min)),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("file_list")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for i in 0..self.files.len() {
                                if self.file_row(ui, i) {
                                    self.select_file(i);
                                }
                            }
                        });
                },
            );
        }

        if open_clicked {
            self.open_folder();
        }
    }

    fn file_row(&self, ui: &mut egui::Ui, index: usize) -> bool {
        let width = ui.available_width();
        let (response, painter) = ui.allocate_painter(vec2(width, 64.0), Sense::click());
        let row = response.rect;
        let selected = self.cursor == Some(index);
        let inner = row.shrink2(vec2(2.0, 1.0));

        if selected {
            painter.rect_filled(inner, 9.0, ACCENT_SOFT);
            painter.rect_stroke(inner, 9.0, Stroke::new(1.0, LINE), StrokeKind::Inside);
        } else if response.hovered() {
            painter.rect_filled(inner, 9.0, HOVER);
        }

        let thumb = Rect::from_min_size(
            pos2(row.left() + 8.0, row.center().y - 24.0),
            vec2(64.0, 48.0),
        );
        painter.rect_filled(thumb, 6.0, Color32::from_rgb(233, 225, 209));
        match &self.thumb_textures[index] {
            Some(tex) => {
                let uv = match self.thumb_bases[index].as_ref() {
                    Some(base) => {
                        cover_uv(vec2(base.width as f32, base.height as f32), thumb.size())
                    }
                    None => Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                };
                egui::Image::from_texture(tex)
                    .uv(uv)
                    .corner_radius(6.0)
                    .maintain_aspect_ratio(false)
                    .fit_to_exact_size(thumb.size())
                    .paint_at(ui, thumb);
            }
            None => {
                painter.text(
                    thumb.center(),
                    Align2::CENTER_CENTER,
                    "...",
                    FontId::proportional(15.0),
                    DIM,
                );
            }
        }
        let border = if selected {
            Stroke::new(1.0, ACCENT_LINE)
        } else {
            Stroke::new(1.0, LINE)
        };
        painter.rect_stroke(thumb, 6.0, border, StrokeKind::Inside);

        let text_clip = Rect::from_min_max(
            pos2(thumb.right() + 11.0, row.top() + 8.0),
            pos2(row.right() - 20.0, row.bottom() - 8.0),
        );
        let text_painter = painter.with_clip_rect(text_clip);
        let name_color = if selected { TEXT } else { MUTED };
        text_painter.text(
            pos2(thumb.right() + 11.0, row.center().y - 9.0),
            Align2::LEFT_CENTER,
            file_name(&self.files[index]),
            FontId::proportional(14.0),
            name_color,
        );
        let meta = self
            .thumb_sizes
            .get(index)
            .copied()
            .flatten()
            .map(|(w, h)| format!("{} × {}", w, h))
            .unwrap_or_else(|| file_extension(&self.files[index]));
        text_painter.text(
            pos2(thumb.right() + 11.0, row.center().y + 9.0),
            Align2::LEFT_CENTER,
            meta,
            FontId::monospace(11.5),
            DIM,
        );

        if self.is_edited_at(index) {
            painter.circle_filled(pos2(row.right() - 11.0, row.center().y), 3.0, ACCENT);
            painter.circle_filled(
                pos2(row.right() - 11.0, row.center().y),
                6.0,
                Color32::from_rgba_unmultiplied(169, 84, 43, 24),
            );
        }

        response.on_hover_text(self.files[index].as_str()).clicked()
    }
}

fn stat_box(ui: &mut egui::Ui, label: &str, value: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(255, 252, 245, 140))
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(6.0)
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(label).size(10.5).strong().color(DIM));
            ui.label(RichText::new(value).size(12.5).monospace().color(TEXT));
        });
}
