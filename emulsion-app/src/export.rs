//! Export dialog and file writers.

use eframe::egui;
use egui::{Align2, Color32, RichText, Stroke, vec2};
use emulsion_core::params::PipelineParams;
use emulsion_core::tonemap::develop;

use crate::app::EmulsionApp;
use crate::theme::{DIM, LINE, MUTED};

impl EmulsionApp {
    pub(crate) fn export_modal(&mut self, ui: &mut egui::Ui) {
        if !self.export_dialog {
            return;
        }
        egui::Window::new("Export")
            .collapsible(false)
            .resizable(false)
            .max_width(370.0)
            .default_width(370.0)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("JPEG quality").size(12.5).color(DIM));
                        ui.spacing_mut().slider_width = ui.available_width();
                        ui.add(
                            egui::Slider::new(&mut self.export_quality, 1..=100)
                                .show_value(false)
                                .integer()
                                .step_by(1.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("This photo").size(13.0))
                                    .min_size(vec2(110.0, 30.0)),
                            )
                            .clicked()
                        {
                            self.export_dialog = false;
                            self.export();
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("All edited photos").size(13.0))
                                    .min_size(vec2(150.0, 30.0)),
                            )
                            .clicked()
                        {
                            self.export_dialog = false;
                            self.export_all_edited();
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Cancel").size(13.0).color(MUTED))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, LINE))
                                    .min_size(vec2(80.0, 30.0)),
                            )
                            .clicked()
                        {
                            self.export_dialog = false;
                        }
                    });
                });
            });
    }

    fn export(&mut self) {
        let Some(i) = self.cursor else { return };
        if i >= self.files.len() {
            return;
        }
        let source = self.files[i].clone();
        self.status = format!("developing full-res {}", source);
        let full = match emulsion_core::decode::decode(source.as_str()) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("export decode error: {}", e.message());
                self.show_toast(self.status.clone());
                return;
            }
        };
        let out = develop(&full, &self.params);

        let stem = std::path::Path::new(&source)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| String::from("emulsion-export"));
        let choice = rfd::FileDialog::new()
            .set_directory(&source)
            .add_filter("JPEG", &["jpg"])
            .add_filter("PNG", &["png"])
            .set_file_name(format!("{}.jpg", stem))
            .save_file();
        let Some(dest) = choice else {
            self.status = String::from("export cancelled");
            return;
        };
        let s = dest.to_string_lossy().into_owned();
        let lower = s.to_lowercase();
        let result = if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
            emulsion_core::export::save_jpeg(s.as_str(), &out, self.export_quality)
        } else {
            emulsion_core::export::save_png(s.as_str(), &out)
        };
        match result {
            Ok(()) => {
                self.status = format!("exported {}", s);
                self.show_toast(self.status.clone());
            }
            Err(e) => {
                self.status = format!("export error: {}", e.message());
                self.show_toast(self.status.clone());
            }
        }
    }

    fn export_all_edited(&mut self) {
        let targets: Vec<(usize, PipelineParams)> = (0..self.files.len())
            .map(|i| (i, self.params_for_index(i)))
            .filter(|(_, p)| *p != PipelineParams::default())
            .collect();
        if targets.is_empty() {
            self.status = String::from("no edited photos to export");
            self.show_toast(self.status.clone());
            return;
        }
        let Some(dir) = rfd::FileDialog::new().pick_folder() else {
            self.status = String::from("export cancelled");
            return;
        };
        let dir = dir.to_string_lossy().into_owned();
        let mut done = 0usize;
        let mut failed = 0usize;
        for (i, params) in targets {
            let source = self.files[i].clone();
            let Ok(full) = emulsion_core::decode::decode(source.as_str()) else {
                failed += 1;
                continue;
            };
            let out = develop(&full, &params);
            let stem = std::path::Path::new(&source)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| String::from("emulsion"));
            let dest = std::path::Path::new(&dir)
                .join(format!("{}.png", stem))
                .to_string_lossy()
                .into_owned();
            match emulsion_core::export::save_png(dest.as_str(), &out) {
                Ok(()) => done += 1,
                Err(_) => failed += 1,
            }
        }
        self.status = format!("exported {} photo(s), {} failed to {}", done, failed, dir);
        self.show_toast(self.status.clone());
    }
}
