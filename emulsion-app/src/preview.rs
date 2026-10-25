//! Preview and thumbnail texture updates.

use eframe::egui;
use egui::vec2;
use emulsion_core::tonemap::{develop, develop_into};

use crate::app::EmulsionApp;
use crate::histogram::make_histogram;
use crate::model::Tool;

impl EmulsionApp {
    pub(crate) fn sync_thumbnails(&mut self, ctx: &egui::Context) {
        if self.thumbs_pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.thumbs_pending);
        for i in pending {
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

        // Develop into the persistent buffers — a slider tick allocates nothing.
        develop_into(
            source_img,
            &preview_params,
            &mut self.preview_scratch,
            &mut self.preview_out,
        );
        let out = &self.preview_out;
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
        // The stage fits and stretches the texture to `texture_size`'s aspect, so
        // it must carry the *developed* output's aspect — `develop` already
        // applied crop, 90° rotation, fine angle and flip. The dims are
        // normalized to the downscaled base's longest edge: a draft (1600) and
        // a crisp (2000) render of the same image must map to the same canvas
        // geometry, or the picture visibly shrinks while a slider is held and
        // pops back on release.
        let longest_ref = loaded.base_small.width.max(loaded.base_small.height) as f32;
        let longest_out = out.width.max(out.height) as f32;
        let scale = longest_ref / longest_out;
        self.texture_size = Some(vec2(out.width as f32 * scale, out.height as f32 * scale));

        // If we were dragging, keep self.dirty = true so that the moment the user
        // lets go of the mouse, it immediately triggers one final high-res render!
        if is_dragging {
            ui.ctx().request_repaint();
        } else {
            self.dirty = false;
            // The selected row's thumbnail follows the live params.
            if let Some(i) = self.cursor {
                self.thumbs_pending.push(i);
            }
        }
    }
}
