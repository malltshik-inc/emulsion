//! Preview and thumbnail texture updates.

use eframe::egui;
use egui::vec2;
use emulsion_core::tonemap::develop;

use crate::app::EmulsionApp;
use crate::histogram::make_histogram;
use crate::model::Tool;

impl EmulsionApp {
    pub(crate) fn sync_thumbnails(&mut self, ctx: &egui::Context) {
        if !self.thumbs_dirty {
            return;
        }
        for i in 0..self.thumb_bases.len() {
            let params = self.params_for_index(i);
            let Some(base) = &self.thumb_bases[i] else {
                continue;
            };
            let cached = self.thumb_params.get(i).copied().flatten();
            if cached == Some(params) && self.thumb_textures[i].is_some() {
                continue;
            }
            let out = develop(base, &params);
            let size = [out.width, out.height];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, &out.data);
            match &mut self.thumb_textures[i] {
                Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
                None => {
                    let name = format!("emulsion-thumb-{}", i);
                    let handle = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
                    self.thumb_textures[i] = Some(handle);
                }
            }
            if let Some(slot) = self.thumb_params.get_mut(i) {
                *slot = Some(params);
            }
        }
        self.thumbs_dirty = false;
    }

    pub(crate) fn sync_texture(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &self.loaded else { return };
        if !self.dirty {
            return;
        }

        let mut preview_params = self.params;
        if self.open_tool == Some(Tool::Crop) {
            preview_params.crop = None;
        }

        // If the user is currently holding down the mouse (dragging a slider/control),
        // use the ultra-fast draft image. Otherwise, use the crisp base_small image.
        let is_dragging = ui.input(|i| i.pointer.any_down());
        let source_img = if is_dragging {
            &loaded.base_draft
        } else {
            &loaded.base_small
        };

        let out = develop(source_img, &preview_params);
        let w = out.width as u32;
        let h = out.height as u32;

        // Only update the heavy histogram on mouse release to save extra CPU cycles during drag
        if !is_dragging {
            self.histogram = make_histogram(&out.data);
        }

        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &out.data);

        match &mut self.texture {
            Some(tex) => {
                tex.set(image, egui::TextureOptions::LINEAR);
            }
            None => {
                let handle =
                    ui.ctx()
                        .load_texture("emulsion-preview", image, egui::TextureOptions::LINEAR);
                self.texture = Some(handle);
            }
        }
        let (base_w, base_h) = (
            loaded.base_small.width as f32,
            loaded.base_small.height as f32,
        );
        let size_swapped = !self.params.rotate_90.is_multiple_of(2);

        self.texture_size = Some(if size_swapped {
            vec2(base_h, base_w)
        } else {
            vec2(base_w, base_h)
        });

        // If we were dragging, keep self.dirty = true so that the moment the user
        // lets go of the mouse, it immediately triggers one final high-res render!
        if is_dragging {
            ui.ctx().request_repaint();
        } else {
            self.dirty = false;
            self.thumbs_dirty = true;
        }
    }
}
