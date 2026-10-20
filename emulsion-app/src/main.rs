#![windows_subsystem = "windows"]

use eframe::egui;
use egui::{Color32, FontId, Pos2, Rect, Sense, Vec2, pos2, vec2};

use emulsion_core::{params::PipelineParams, tonemap::develop};

use std::sync::mpsc::{Receiver, channel};

const IMAGE_EXTS: [&str; 6] = [".orf", ".jpg", ".jpeg", ".png", ".tif", ".tiff"];

/// A decoded image ready for interactive editing: the app keeps a downscaled
/// copy of the developed base for fast preview renders; the full-res source
/// is re-decoded only for export.
struct Loaded {
    base_small: emulsion_core::image::LinearImage,
}

/// Case-insensitive substring match for the tools-panel filter.
fn filter_match(query: &str, label: &str) -> bool {
    if query.len() == 0 {
        return true;
    }
    let h = label.to_lowercase();
    let n = query.to_lowercase();
    h.as_str().match_indices(n.as_str()).next().is_some()
}

struct EmulsionApp {
    folder: String,
    files: Vec<String>,
    cursor: Option<usize>,
    loaded: Option<Loaded>,

    load_rx: Option<Receiver<Result<Loaded, String>>>,
    loading: bool,

    params: PipelineParams,
    search: String,
    texture: Option<egui::TextureHandle>,
    texture_size: Option<Vec2>,
    zoom: f32,
    pan: Vec2,

    status: String,
    dirty: bool,
}

impl EmulsionApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            folder: String::new(),
            files: Vec::new(),
            cursor: None,
            loaded: None,
            load_rx: None,
            loading: false,
            params: PipelineParams::default(),
            search: String::new(),
            texture: None,
            texture_size: None,
            zoom: 0.0,
            pan: Vec2::ZERO,
            status: String::from("Open a folder to start"),
            dirty: true,
        }
    }

    fn list_images(path: &str) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let mut queue: Vec<String> = vec![String::from(path)];
        while let Some(dir) = queue.pop() {
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    for raw in entries {
                        match raw {
                            Ok(entry) => {
                                let p = entry.path();
                                if p.is_dir() {
                                    queue.push(String::from(p.to_string_lossy().into_owned()));
                                } else {
                                    let s = String::from(p.to_string_lossy().into_owned());
                                    let lower = s.to_lowercase();
                                    if IMAGE_EXTS.iter().any(|it| lower.ends_with(*it)) {
                                        out.push(s);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => return Err(format!("cannot read {}: {}", dir, e)),
            }
        }
        out.sort();
        Ok(out)
    }

    fn open_folder(&mut self) {
        let dir = rfd::FileDialog::new().pick_folder();
        match dir {
            Some(p) => {
                let path = String::from(p.to_path_buf().into_string().unwrap());
                self.folder = path.clone();
                self.cursor = None;
                self.loaded = None;
                self.dirty = true;
                match Self::list_images(path.as_str()) {
                    Ok(files) => {
                        self.files = files;
                        self.status = format!("{} images in folder", self.files.len());
                    }
                    Err(e) => self.status = format!("error: {}", e),
                }
            }
            None => {}
        }
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
                    sender
                        .send(Err(format!("decode failed: {}", e.message())))
                        .unwrap();
                    return;
                }
            };
            let base_small = emulsion_core::geometry::downscale(&base, 1800);
            sender.send(Ok(Loaded { base_small })).unwrap();
        });
    }

    fn poll_load(&mut self) {
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
                    self.pan = Vec2::ZERO;
                    self.load_rx = None;
                }
                Err(_) => return,
            }
        }
    }

    fn sync_texture(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &self.loaded else { return };
        if !self.dirty {
            return;
        }
        let out = develop(&loaded.base_small, &self.params);
        let w = out.width as u32;
        let h = out.height as u32;
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
        self.texture_size = Some(vec2(w as f32, h as f32));
        self.dirty = false;
    }

    fn export(&mut self) {
        let Some(i) = self.cursor else { return };
        if i >= self.files.len() {
            return;
        }
        let path = &self.files[i];
        self.status = format!("developing full-res {}", path);
        let full = match emulsion_core::decode::decode(path.as_str()) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("export decode error: {}", e.message());
                return;
            }
        };
        let out = develop(&full, &self.params);
        let choice = rfd::FileDialog::new()
            .set_directory(&path)
            .add_filter("PNG", &["png"])
            .add_filter("JPEG", &["jpg"])
            .set_file_name("emulsion-export")
            .save_file();
        match choice {
            Some(dest) => {
                let s = String::from(dest.to_path_buf().into_string().unwrap());
                let lower = s.to_lowercase();
                let result = if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
                    emulsion_core::export::save_jpeg(s.as_str(), &out, 92)
                } else {
                    emulsion_core::export::save_png(s.as_str(), &out)
                };
                match result {
                    Ok(()) => self.status = format!("exported {}", s),
                    Err(e) => self.status = format!("export error: {}", e.message()),
                }
            }
            None => {}
        }
    }

    // ---------------------------------------------------------------- panels

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Export").clicked() {
                self.export();
            }
        });
    }

    fn file_panel(&mut self, ui: &mut egui::Ui) {
        ui.take_available_space();
        if ui.button("Open folder…").clicked() {
            self.open_folder();
        }
        self.preview_pane(ui);
        egui::ScrollArea::both().show(ui, |ui| {
            if self.files.is_empty() {
                ui.label("No images yet — open a folder");
            }
            for i in 0..self.files.len() {
                let label = &self.files[i];
                if ui.button(format!("{}", *label)).clicked() {
                    self.cursor = Some(i);
                    self.load_selected();
                }
            }
        });
    }

    fn preview_pane(&self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(
            vec2(ui.available_size_before_wrap().x, 150.0),
            Sense::hover(),
        );
        painter.rect_filled(response.rect, 0.0, Color32::from_black_alpha(8));
        let Some(size) = &self.texture_size else {
            painter.text(
                pos2(12.0, 12.0),
                egui::Align2::LEFT_TOP,
                format!("{}", self.status),
                FontId::proportional(13.0),
                Color32::WHITE,
            );
            return;
        };
        let Some(tex) = &self.texture else { return };

        let area = response.rect;
        let av = area.size();
        let scale = (av.x / size.x).min(av.y / size.y) * 0.92;
        let w = size.x * scale;
        let h = size.y * scale;
        let rect = Rect::from_min_size(
            pos2(
                area.left() + (av.x - w) / 2.0,
                area.top() + (av.y - h) / 2.0,
            ),
            vec2(w, h),
        );
        painter.image(
            tex.id(),
            rect,
            Rect::from_min_size(Pos2::ZERO, vec2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    fn tools_panel(&mut self, ui: &mut egui::Ui) {
        ui.take_available_space();
        let changed = egui::ScrollArea::both()
            .show(ui, |ui| {
                let mut changed = false;

                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("settings…")
                        .desired_width(150.0)
                        .show(ui);
                });

                let query = String::from(self.search.as_str());
                let searching = query.len() != 0;
                let p = &mut self.params;

                let color_on = !searching
                    || filter_match(query.as_str(), "WB temperature")
                    || filter_match(query.as_str(), "Exposure")
                    || filter_match(query.as_str(), "Black point");
                if color_on {
                    egui::CollapsingHeader::new("Color")
                        .default_open(true)
                        .open(if searching { Some(true) } else { None })
                        .show(ui, |ui| {
                            if filter_match(query.as_str(), "WB temperature") {
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut p.wb_temperature, -100.0..=100.0)
                                            .text("WB temperature"),
                                    )
                                    .changed();
                            }
                            if filter_match(query.as_str(), "WB tint") {
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut p.wb_tint, -100.0..=100.0)
                                            .text("WB tint"),
                                    )
                                    .changed();
                            }
                            if filter_match(query.as_str(), "Exposure") {
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut p.exposure, -3.0..=3.0)
                                            .text("Exposure (EV)"),
                                    )
                                    .changed();
                            }
                            if filter_match(query.as_str(), "Black point R") {
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut p.black_point[0], 0.0..=1.0)
                                            .text("Black point R"),
                                    )
                                    .changed();
                            }
                            if filter_match(query.as_str(), "Black point G") {
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut p.black_point[1], 0.0..=1.0)
                                            .text("Black point G"),
                                    )
                                    .changed();
                            }
                            if filter_match(query.as_str(), "Black point B") {
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut p.black_point[2], 0.0..=1.0)
                                            .text("Black point B"),
                                    )
                                    .changed();
                            }
                        });
                }

                let convert_on = !searching
                    || filter_match(query.as_str(), "Convert")
                    || filter_match(query.as_str(), "Negative");
                if convert_on {
                    egui::CollapsingHeader::new("Convert")
                        .default_open(true)
                        .open(if searching { Some(true) } else { None })
                        .show(ui, |ui| {
                            if filter_match(query.as_str(), "Negative") {
                                changed |=
                                    ui.checkbox(&mut p.invert, "Negative → positive").changed();
                            }
                        });
                }

                let rotate_on = !searching
                    || filter_match(query.as_str(), "Rotate")
                    || filter_match(query.as_str(), "90")
                    || filter_match(query.as_str(), "Fine angle");
                if rotate_on {
                    egui::CollapsingHeader::new("Rotate")
                        .default_open(true)
                        .open(if searching { Some(true) } else { None })
                        .show(ui, |ui| {
                            if filter_match(query.as_str(), "Rotate")
                                || filter_match(query.as_str(), "90")
                            {
                                ui.horizontal(|ui| {
                                    if ui.button("⟲ 90").clicked() {
                                        p.rotate_90 = (p.rotate_90 + 1_u8) % 4_u8;
                                        changed = true;
                                    }
                                    if ui.button("⟳ 90").clicked() {
                                        p.rotate_90 = (p.rotate_90 + 3_u8) % 4_u8;
                                        changed = true;
                                    }
                                });
                            }
                            if filter_match(query.as_str(), "Fine angle") {
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut p.fine_angle, -5.0..=5.0)
                                            .text("Fine angle (°)"),
                                    )
                                    .changed();
                            }
                        });
                }

                changed
            })
            .inner;
        if changed {
            self.dirty = true;
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.label(format!("{}", self.status));
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::hover());

        let texture_opt = &self.texture;
        if texture_opt.is_none() {
            painter.rect_filled(response.rect, 0.0, Color32::from_black_alpha(8));
            painter.text(
                pos2(12.0, 12.0),
                egui::Align2::LEFT_TOP,
                format!("{}", self.status),
                FontId::proportional(13.0),
                Color32::WHITE,
            );
            return;
        }
        let Some(size) = &self.texture_size else {
            return;
        };
        let img_w = size.x;
        let img_h = size.y;
        let area = response.rect;
        let av = area.size();
        if self.zoom <= 0.0 {
            self.zoom = (av.x / img_w).min(av.y / img_h);
        }
        let scale = self.zoom;
        let w = img_w * scale;
        let h = img_h * scale;
        let cx = area.center().x + self.pan.x;
        let cy = area.center().y + self.pan.y;
        let rect = Rect::from_min_size(pos2(cx - w / 2.0, cy - h / 2.0), vec2(w, h));

        let Some(tex) = &self.texture else { return };
        painter.image(
            tex.id(),
            rect,
            Rect::from_min_size(Pos2::ZERO, vec2(1.0, 1.0)),
            Color32::WHITE,
        );
        painter.rect_filled(response.rect, 0.0, Color32::from_black_alpha(8));

        let scroll = ui.input(|i| i.smooth_scroll_delta().y);
        if response.hovered() && scroll != 0.0 {
            self.zoom *= 1.0 + scroll * 0.005;
            ui.ctx().request_repaint();
        }
        if ui.input(|i| i.pointer.primary_down() || i.pointer.middle_down()) {
            let delta = ui.input(|i| i.pointer.delta());
            self.pan = self.pan + delta;
        }
    }
}

impl eframe::App for EmulsionApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Handle any completed background loads + re-render preview texture.
        self.poll_load();
        self.sync_texture(ui);

        egui::Panel::top("top_panel")
            .exact_size(30.0)
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::left("file_panel")
            .default_size(280.0)
            .min_size(200.0)
            .show(ui, |ui| self.file_panel(ui));

        egui::Panel::right("tools_panel")
            .default_size(300.0)
            .min_size(240.0)
            .show(ui, |ui| self.tools_panel(ui));

        egui::Panel::bottom("status_panel")
            .exact_size(26.0)
            .show(ui, |ui| self.status_bar(ui));

        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
    }
}

fn main() {
    let options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "Emulsion",
        options,
        Box::new(|cc| Ok(Box::new(EmulsionApp::new(&cc)))),
    );
    std::process::exit(0);
}
