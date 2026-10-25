//! Folder scanning, file selection and background decoding.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;

use eframe::egui;
use egui::Vec2;
use emulsion_core::params::PipelineParams;

use crate::app::EmulsionApp;
use crate::model::{CropDrag, Loaded, Orientation, ThumbData};
use crate::theme::THUMB_EDGE;

impl EmulsionApp {
    fn list_images(path: &str) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let mut queue: Vec<String> = vec![String::from(path)];
        while let Some(dir) = queue.pop() {
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            queue.push(p.to_string_lossy().into_owned());
                        } else {
                            let s = p.to_string_lossy().into_owned();
                            if emulsion_core::decode::is_supported(&s) {
                                out.push(s);
                            }
                        }
                    }
                }
                Err(e) => return Err(format!("cannot read {}: {}", dir, e)),
            }
        }
        out.sort();
        Ok(out)
    }

    pub(crate) fn open_folder(&mut self) {
        let dir = rfd::FileDialog::new().pick_folder();
        if let Some(p) = dir {
            let path = p.to_string_lossy().into_owned();
            self.save_current();
            self.folder = path.clone();
            self.cursor = None;
            self.loaded = None;
            self.texture = None;
            self.texture_size = None;
            self.histogram = None;
            self.params = PipelineParams::default();
            self.film_textures.clear();
            self.film_preview_base = None;
            self.film_preview_params = None;
            self.film_preview_source = None;
            self.dirty = true;
            match Self::list_images(path.as_str()) {
                Ok(files) => {
                    self.files = files;
                    self.status = format!("{} images in folder", self.files.len());
                }
                Err(e) => {
                    self.files = Vec::new();
                    self.status = format!("error: {}", e);
                }
            }
            self.start_thumbnails();
        }
    }

    fn save_current(&mut self) {
        if let Some(current) = self.cursor
            && let Some(path) = self.files.get(current)
        {
            self.settings.insert(path.clone(), self.params);
        }
    }

    pub(crate) fn select_file(&mut self, index: usize) {
        self.save_current();
        self.cursor = Some(index);
        self.params = self
            .files
            .get(index)
            .and_then(|path| self.settings.get(path))
            .copied()
            .unwrap_or_default();
        self.crop_rect = self.params.crop;
        self.crop_drag = CropDrag::None;
        self.crop_drag_start = None;
        self.crop_drag_rect = None;
        if let Some((w, h)) = self.current_size() {
            self.crop_orientation = if w >= h {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
        }
        self.loaded = None;
        self.texture = None;
        self.texture_size = None;
        self.histogram = None;
        self.film_textures.clear();
        self.film_preview_base = None;
        self.film_preview_params = None;
        self.film_preview_source = None;
        self.dirty = true;
        self.load_selected();
    }

    fn load_selected(&mut self) {
        let Some(i) = self.cursor else { return };
        if i >= self.files.len() {
            return;
        }
        let path = &self.files[i];
        self.loading = true;
        self.status = format!("loading {}", path);
        let (sender, receiver) = channel::<Result<Loaded, String>>();
        self.load_rx = Some(receiver);
        let path_owned = String::from(path.as_str());
        std::thread::spawn(move || {
            let base = match emulsion_core::decode::decode(path_owned.as_str()) {
                Ok(b) => b,
                Err(e) => {
                    let _ = sender.send(Err(format!("decode failed: {}", e.message())));
                    return;
                }
            };
            let source_size = (base.width, base.height);
            let base_small = emulsion_core::geometry::downscale(&base, 2000);
            let base_draft = emulsion_core::geometry::downscale(&base, 1000);
            let _ = sender.send(Ok(Loaded {
                base_small,
                base_draft,
                source_size,
            }));
        });
    }

    pub(crate) fn poll_load(&mut self) {
        loop {
            let Some(rx) = &self.load_rx else { return };
            match rx.try_recv() {
                Ok(msg) => {
                    match msg {
                        Ok(loaded) => {
                            self.loaded = Some(loaded);
                            self.status = String::from("ready — adjust the sliders");
                        }
                        Err(e) => self.status = e,
                    }
                    self.loading = false;
                    self.dirty = true;
                    self.zoom = 0.0;
                    self.fit_zoom = 0.0;
                    self.pan = Vec2::ZERO;
                    self.load_rx = None;
                }
                Err(_) => return,
            }
        }
    }

    fn start_thumbnails(&mut self) {
        if let Some(flag) = &self.thumb_cancel {
            flag.store(true, Ordering::Relaxed);
        }
        self.thumb_generation = self.thumb_generation.wrapping_add(1);
        let generation = self.thumb_generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.thumb_cancel = Some(Arc::clone(&cancel));
        self.thumb_bases = vec![None; self.files.len()];
        self.thumb_textures = vec![None; self.files.len()];
        self.thumb_sizes = vec![None; self.files.len()];
        self.thumb_params = vec![None; self.files.len()];
        self.thumbs_dirty = true;

        let files = self.files.clone();
        let (sender, receiver) = channel();
        self.thumb_rx = Some(receiver);
        std::thread::spawn(move || {
            for (index, path) in files.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                let base = match emulsion_core::decode::decode(path.as_str()) {
                    Ok(base) => base,
                    Err(_) => continue,
                };
                let source_size = (base.width, base.height);
                let small = emulsion_core::geometry::downscale(&base, THUMB_EDGE);
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                if sender
                    .send((
                        generation,
                        Some(ThumbData {
                            index,
                            base: small,
                            source_size,
                        }),
                    ))
                    .is_err()
                {
                    return;
                }
            }
            let _ = sender.send((generation, None));
        });
    }

    pub(crate) fn poll_thumbnails(&mut self) {
        loop {
            let Some(rx) = &self.thumb_rx else { return };
            match rx.try_recv() {
                Ok((generation, message)) => {
                    if generation != self.thumb_generation {
                        continue;
                    }
                    match message {
                        Some(data) => {
                            if data.index < self.thumb_bases.len() {
                                self.thumb_bases[data.index] = Some(data.base);
                                self.thumb_sizes[data.index] = Some(data.source_size);
                                self.thumbs_dirty = true;
                            }
                        }
                        None => {
                            self.thumb_rx = None;
                            return;
                        }
                    }
                }
                Err(_) => return,
            }
        }
    }

    pub(crate) fn params_for_index(&self, index: usize) -> PipelineParams {
        if self.cursor == Some(index) {
            self.params
        } else {
            self.files
                .get(index)
                .and_then(|path| self.settings.get(path))
                .copied()
                .unwrap_or_default()
        }
    }

    pub(crate) fn current_size(&self) -> Option<(usize, usize)> {
        let index = self.cursor?;
        if let Some(loaded) = &self.loaded {
            return Some(loaded.source_size);
        }
        self.thumb_sizes.get(index).copied().flatten()
    }

    pub(crate) fn current_name(&self) -> String {
        self.cursor
            .and_then(|index| self.files.get(index))
            .map(|path| crate::util::file_stem(path))
            .unwrap_or_else(|| String::from("No image"))
    }

    pub(crate) fn current_meta(&self) -> String {
        let Some(index) = self.cursor else {
            return String::from("OPEN A FOLDER");
        };
        let Some(path) = self.files.get(index) else {
            return String::from("OPEN A FOLDER");
        };
        let extension = std::path::Path::new(path)
            .extension()
            .map(|value| value.to_string_lossy().to_uppercase())
            .unwrap_or_else(|| String::from("IMAGE"));
        match self.current_size() {
            Some((w, h)) => format!(
                "{} · {:.1} MP",
                extension,
                w as f64 * h as f64 / 1_000_000.0
            ),
            None => extension,
        }
    }

    pub(crate) fn is_edited_at(&self, index: usize) -> bool {
        self.params_for_index(index) != PipelineParams::default()
    }

    pub(crate) fn is_current_edited(&self) -> bool {
        self.params != PipelineParams::default()
    }
}
