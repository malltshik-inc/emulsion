//! Library sidebar: file list, thumbnails and histogram/stats footer.

use eframe::egui;
use egui::{
    pos2, vec2, Align, Align2, Color32, FontId, Layout, Margin, Pos2, Rect, RichText, Sense,
    Stroke, StrokeKind,
};

use crate::app::EmulsionApp;
use crate::histogram::paint_histogram;
use crate::model::{RowAction, TreeNode};
use crate::theme::{ACCENT, ACCENT_LINE, ACCENT_SOFT, DIM, HOVER, LINE, MUTED, TEXT};
use crate::util::{aspect_label, cover_uv, file_extension, file_name};

/// Widget feedback collected during the immutable tree render, applied after.
#[derive(Default)]
struct TreeEvents {
    clicked: Option<usize>,
    toggles: Vec<String>,
    action: Option<(RowAction, usize)>,
}

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
        if self.tree.is_empty() {
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
                    egui::ScrollArea::both()
                        .id_salt("file_list")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let viewport = ui.available_width();
                            let mut events = TreeEvents::default();
                            self.render_tree(ui, &self.tree, 0, viewport, &mut events);
                            // Only a real folder toggle changes which leaves are
                            // visible; re-scheduling on every frame would cancel the
                            // decode worker before it could finish slow RAWs.
                            if !events.toggles.is_empty() {
                                for path in events.toggles {
                                    if !self.expanded.remove(&path) {
                                        self.expanded.insert(path);
                                    }
                                }
                                self.schedule_visible_thumbnails();
                            }
                            if let Some(i) = events.clicked {
                                self.select_file(i);
                            }
                            match events.action {
                                Some((RowAction::Copy, index)) => self.copy_settings_from(index),
                                Some((RowAction::Paste, index)) => self.paste_settings_to(index),
                                None => {}
                            }
                        });
                },
            );
        }

        if open_clicked {
            self.open_folder();
        }
    }

    fn render_tree(
        &self,
        ui: &mut egui::Ui,
        nodes: &[TreeNode],
        depth: usize,
        viewport: f32,
        events: &mut TreeEvents,
    ) {
        for node in nodes {
            if let Some(index) = node.file_index {
                if self.file_row(ui, index, depth, viewport, events) {
                    events.clicked = Some(index);
                }
            } else {
                let is_open = self.expanded.contains(&node.path);
                if self.folder_row(ui, node, is_open, depth, viewport) {
                    events.toggles.push(node.path.clone());
                }
                if is_open {
                    self.render_tree(ui, &node.children, depth + 1, viewport, events);
                }
            }
        }
    }

    fn folder_row(
        &self,
        ui: &mut egui::Ui,
        node: &TreeNode,
        is_open: bool,
        depth: usize,
        viewport: f32,
    ) -> bool {
        let indent = 8.0 + depth as f32 * 16.0;
        // One layout pass: the galley is measured and then painted as-is.
        let name_galley = ui.ctx().fonts_mut(|fonts| {
            fonts.layout_no_wrap(node.name.clone(), FontId::proportional(14.0), MUTED)
        });
        let name_w = name_galley.rect.width();
        let width = viewport.max(indent + 16.0 + name_w + 16.0);
        let (response, painter) = ui.allocate_painter(vec2(width, 30.0), Sense::click());
        let row = response.rect;
        let inner = row.shrink2(vec2(2.0, 1.0));
        if response.hovered() {
            painter.rect_filled(inner, 7.0, HOVER);
        }
        painter.text(
            pos2(row.left() + indent, row.center().y),
            Align2::LEFT_CENTER,
            if is_open { "\u{25be}" } else { "\u{25b8}" },
            FontId::proportional(13.0),
            DIM,
        );
        let name_rect = Rect::from_min_max(
            pos2(row.left() + indent + 16.0, row.top() + 2.0),
            pos2(row.right() - 8.0, row.bottom() - 2.0),
        );
        let name_pos = pos2(row.left() + indent + 16.0, row.center().y);
        painter.with_clip_rect(name_rect).galley(
            pos2(name_pos.x, name_pos.y - name_galley.rect.height() / 2.0),
            name_galley,
            MUTED,
        );
        response.on_hover_text(node.path.as_str()).clicked()
    }

    fn file_row(
        &self,
        ui: &mut egui::Ui,
        index: usize,
        depth: usize,
        viewport: f32,
        events: &mut TreeEvents,
    ) -> bool {
        let indent = 8.0 + depth as f32 * 16.0;
        let selected = self.cursor == Some(index);
        let name_color = if selected { TEXT } else { MUTED };
        // One layout pass: the galley is measured and then painted as-is.
        let name_galley = ui.ctx().fonts_mut(|fonts| {
            fonts.layout_no_wrap(
                file_name(&self.files[index]),
                FontId::proportional(14.0),
                name_color,
            )
        });
        let name_w = name_galley.rect.width();
        let width = viewport.max(indent + 64.0 + 11.0 + name_w + 20.0);
        let (response, painter) = ui.allocate_painter(vec2(width, 64.0), Sense::click());
        let row = response.rect;
        let inner = row.shrink2(vec2(2.0, 1.0));

        if selected {
            painter.rect_filled(inner, 9.0, ACCENT_SOFT);
            painter.rect_stroke(inner, 9.0, Stroke::new(1.0, LINE), StrokeKind::Inside);
        } else if response.hovered() {
            painter.rect_filled(inner, 9.0, HOVER);
        }

        let thumb = Rect::from_min_size(
            pos2(row.left() + indent, row.center().y - 24.0),
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
        let name_pos = pos2(thumb.right() + 11.0, row.center().y - 9.0);
        text_painter.galley(
            pos2(name_pos.x, name_pos.y - name_galley.rect.height() / 2.0),
            name_galley,
            name_color,
        );
        // Show only the format under a thumbnail; the resolution would read as
        // the small thumb's own dimensions and only confuse (the true source
        // size still lives in the footer/`current_size`).
        let meta = file_extension(&self.files[index]);
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

        let resp = response.on_hover_text(self.files[index].as_str());
        resp.context_menu(|ui| {
            ui.set_min_width(150.0);
            // Copy only what is actually there — an untouched file has nothing.
            if ui
                .add_enabled(
                    self.is_edited_at(index),
                    egui::Button::new("Copy settings").shortcut_text("Ctrl/Cmd+C"),
                )
                .clicked()
            {
                events.action = Some((RowAction::Copy, index));
            }
            if ui
                .add_enabled(
                    self.clipboard.is_some(),
                    egui::Button::new("Paste settings").shortcut_text("Ctrl/Cmd+V"),
                )
                .clicked()
            {
                events.action = Some((RowAction::Paste, index));
            }
        });
        resp.clicked()
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
