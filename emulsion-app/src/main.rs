#![cfg_attr(windows, windows_subsystem = "windows")]

use eframe::egui;
use egui::{
    Align, Align2, Color32, FontFamily, FontId, Layout, Margin, Pos2, Rect, RichText, Sense,
    Stroke, StrokeKind, TextStyle, Vec2, pos2, scroll_area::ScrollBarVisibility, vec2,
};

use emulsion_core::{
    film,
    params::{CropRect, PipelineParams},
    tonemap::develop,
};

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

const THUMB_EDGE: usize = 96;
const SIDEBAR_WIDTH: f32 = 308.0;
const TOPBAR_HEIGHT: f32 = 62.0;
const SETTINGS_HEIGHT: f32 = 70.0;
const SETTINGS_OPEN_HEIGHT: f32 = 126.0;
const SETTINGS_SIMULATIONS_HEIGHT: f32 = 248.0;
const FILM_CARD_WIDTH: f32 = 96.0;
const FILM_CARD_HEIGHT: f32 = 116.0;
const FILM_CARD_GAP: f32 = 8.0;
const FILM_THUMB_HEIGHT: f32 = 66.0;
const FILM_PRELOAD_CARDS: f32 = 2.0;
const FILM_PRELOAD_MARGIN: f32 = FILM_PRELOAD_CARDS * (FILM_CARD_WIDTH + FILM_CARD_GAP);

const BG: Color32 = Color32::from_rgb(242, 236, 221);
const PANEL: Color32 = Color32::from_rgb(250, 246, 236);
const PANEL_TOP: Color32 = Color32::from_rgb(255, 253, 248);
const SETTINGS_BG: Color32 = Color32::from_rgb(247, 241, 230);
const STAGE_IN: Color32 = Color32::from_rgb(251, 247, 238);
const STAGE_OUT: Color32 = Color32::from_rgb(232, 223, 205);
const LINE: Color32 = Color32::from_rgba_unmultiplied_const(56, 44, 28, 28);
const LINE_STRONG: Color32 = Color32::from_rgba_unmultiplied_const(56, 44, 28, 51);
const TEXT: Color32 = Color32::from_rgb(43, 36, 25);
const MUTED: Color32 = Color32::from_rgb(124, 112, 94);
const DIM: Color32 = Color32::from_rgb(167, 155, 133);
const ACCENT: Color32 = Color32::from_rgb(169, 84, 43);
const ACCENT_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(169, 84, 43, 26);
const ACCENT_LINE: Color32 = Color32::from_rgba_unmultiplied_const(169, 84, 43, 77);
const HOVER: Color32 = Color32::from_rgba_unmultiplied_const(56, 44, 28, 13);
const TOAST_BG: Color32 = Color32::from_rgb(43, 36, 25);

type ThumbMessage = (u64, Option<ThumbData>);

struct ThumbData {
    index: usize,
    base: emulsion_core::image::LinearImage,
    source_size: (usize, usize),
}

struct Loaded {
    base_small: emulsion_core::image::LinearImage,
    base_draft: emulsion_core::image::LinearImage,
    source_size: (usize, usize),
}

#[derive(Clone)]
struct Histogram {
    channels: [[f32; 64]; 3],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Develop,
    Simulations,
    Wb,
    Tint,
    Ev,
    Contrast,
    Saturation,
    Shadows,
    Highlights,
    Vignette,
    Rotate,
    Crop,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Develop => "Develop",
            Self::Simulations => "Simulations",
            Self::Wb => "White Balance",
            Self::Tint => "Tint",
            Self::Ev => "Exposure",
            Self::Contrast => "Contrast",
            Self::Saturation => "Saturation",
            Self::Shadows => "Shadows",
            Self::Highlights => "Highlights",
            Self::Vignette => "Vignette",
            Self::Rotate => "Rotate",
            Self::Crop => "Crop",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Develop => "DEVELOP",
            Self::Simulations => "SIMULATIONS",
            Self::Wb => "WB",
            Self::Tint => "TINT",
            Self::Ev => "EV",
            Self::Contrast => "CONTRAST",
            Self::Saturation => "SATURATION",
            Self::Shadows => "SHADOWS",
            Self::Highlights => "HIGHLIGHTS",
            Self::Vignette => "VIGNETTE",
            Self::Rotate => "ROTATE",
            Self::Crop => "CROP",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Develop => "DEVELOP",
            Self::Simulations => "SIMULATION",
            Self::Wb => "WB",
            Self::Tint => "TINT",
            Self::Ev => "EV",
            Self::Contrast => "CONTRAST",
            Self::Saturation => "SATURATION",
            Self::Shadows => "SHADOWS",
            Self::Highlights => "HIGHLIGHTS",
            Self::Vignette => "EDGE",
            Self::Rotate => "ANGLE",
            Self::Crop => "ASPECT",
        }
    }

    fn aliases(self) -> &'static str {
        match self {
            Self::Develop => "develop negative positive invert",
            Self::Simulations => "film simulation lut negative positive invert develop",
            Self::Wb => "white balance temperature warm cool",
            Self::Tint => "tint magenta green",
            Self::Ev => "exposure brightness",
            Self::Contrast => "contrast",
            Self::Saturation => "saturation vibrance color",
            Self::Shadows => "shadows dark lift",
            Self::Highlights => "highlights bright recovery",
            Self::Vignette => "vignette edges darken",
            Self::Rotate => "rotate angle straighten",
            Self::Crop => "crop aspect ratio",
        }
    }

    fn range(self) -> std::ops::RangeInclusive<f32> {
        match self {
            Self::Ev => -3.0..=3.0,
            Self::Rotate => -5.0..=5.0,
            Self::Crop | Self::Develop | Self::Simulations => 0.0..=0.0, // use different UI
            _ => -100.0..=100.0,
        }
    }

    fn decimals(self) -> usize {
        match self {
            Self::Ev | Self::Rotate => 1,
            _ => 0,
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Rotate => "°",
            _ => "",
        }
    }

    fn value_mut(self, params: &mut PipelineParams) -> Option<&mut f32> {
        match self {
            Self::Wb => Some(&mut params.wb_temperature),
            Self::Tint => Some(&mut params.wb_tint),
            Self::Ev => Some(&mut params.exposure),
            Self::Contrast => Some(&mut params.contrast),
            Self::Saturation => Some(&mut params.saturation),
            Self::Shadows => Some(&mut params.shadows),
            Self::Highlights => Some(&mut params.highlights),
            Self::Vignette => Some(&mut params.vignette),
            Self::Rotate => Some(&mut params.fine_angle),
            Self::Crop | Self::Develop | Self::Simulations => None,
        }
    }

    fn changed(self, params: &PipelineParams) -> bool {
        match self {
            Self::Develop => params.invert,
            Self::Simulations => params.film.is_some(),
            Self::Rotate => {
                params.fine_angle != 0.0 || params.rotate_90 != 0 || params.flip_horizontal
            }
            Self::Crop => params.crop.is_some(),
            _ => {
                let mut copy = *params;
                self.value_mut(&mut copy).is_some_and(|t| {
                    *t != 0.0
                })
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CropAspect {
    Free,
    Original,
    Square,
    FourThree,
    ThreeTwo,
    SixteenNine,
    TwoOne,
}

impl CropAspect {
    const ALL: [CropAspect; 7] = [
        Self::Free,
        Self::Original,
        Self::Square,
        Self::FourThree,
        Self::ThreeTwo,
        Self::SixteenNine,
        Self::TwoOne,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Original => "Original",
            Self::Square => "1:1",
            Self::FourThree => "4:3",
            Self::ThreeTwo => "3:2",
            Self::SixteenNine => "16:9",
            Self::TwoOne => "2:1",
        }
    }

    fn is_free(self) -> bool {
        matches!(self, Self::Free)
    }

    /// Base width / height, or `None` for an unconstrained crop.
    fn ratio(self) -> Option<f32> {
        match self {
            Self::Free | Self::Original => None,
            Self::Square => Some(1.0),
            Self::FourThree => Some(4.0 / 3.0),
            Self::ThreeTwo => Some(3.0 / 2.0),
            Self::SixteenNine => Some(16.0 / 9.0),
            Self::TwoOne => Some(2.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CropDrag {
    None,
    Draw, // drawing a fresh rect
    Rect, // dragging whole rect
    TL,
    TR,
    BL,
    BR, // corners
    T,
    B,
    L,
    R, // edges
}

struct EmulsionApp {
    folder: String,
    files: Vec<String>,
    cursor: Option<usize>,
    loaded: Option<Loaded>,

    load_rx: Option<Receiver<Result<Loaded, String>>>,
    loading: bool,

    thumb_bases: Vec<Option<emulsion_core::image::LinearImage>>,
    thumb_textures: Vec<Option<egui::TextureHandle>>,
    thumb_sizes: Vec<Option<(usize, usize)>>,
    thumb_params: Vec<Option<PipelineParams>>,
    thumb_rx: Option<Receiver<ThumbMessage>>,
    thumb_cancel: Option<Arc<AtomicBool>>,
    thumb_generation: u64,
    thumbs_dirty: bool,

    params: PipelineParams,
    settings: HashMap<String, PipelineParams>,
    texture: Option<egui::TextureHandle>,
    texture_size: Option<Vec2>,
    zoom: f32,
    fit_zoom: f32,
    last_stage: Vec2,
    pan: Vec2,

    open_tool: Option<Tool>,
    tool_search: String,
    focus_search: bool,
    film_search: String,
    film_focus_search: bool,
    film_textures: HashMap<Option<u16>, egui::TextureHandle>,
    film_preview_base: Option<emulsion_core::image::LinearImage>,
    film_preview_params: Option<PipelineParams>,
    film_preview_source: Option<usize>,
    histogram: Option<Histogram>,

    logo: Option<egui::TextureHandle>,
    wordmark_family: FontFamily,
    status: String,
    toast: Option<(String, Instant)>,
    dirty: bool,
    clipboard: Option<PipelineParams>,
    export_dialog: bool,

    // Crop state
    crop_rect: Option<CropRect>,      // live crop (mirrors params.crop)
    crop_drag: CropDrag,              // current drag state
    crop_drag_start: Option<Pos2>,    // drag start position
    crop_drag_rect: Option<CropRect>, // rect at drag start
    crop_aspect: CropAspect,          // aspect-ratio constraint
    crop_orientation: Orientation,    // portrait / landscape
    crop_pan: bool,                   // current drag started outside the crop (pan)
}

impl EmulsionApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let wordmark_family = install_fonts(&cc.egui_ctx);
        install_style(&cc.egui_ctx);
        let logo = load_logo(&cc.egui_ctx);
        Self {
            folder: String::new(),
            files: Vec::new(),
            cursor: None,
            loaded: None,
            load_rx: None,
            loading: false,
            thumb_bases: Vec::new(),
            thumb_textures: Vec::new(),
            thumb_sizes: Vec::new(),
            thumb_params: Vec::new(),
            thumb_rx: None,
            thumb_cancel: None,
            thumb_generation: 0,
            thumbs_dirty: false,
            params: PipelineParams::default(),
            settings: HashMap::new(),
            texture: None,
            texture_size: None,
            zoom: 0.0,
            fit_zoom: 0.0,
            last_stage: Vec2::ZERO,
            pan: Vec2::ZERO,
            open_tool: None,
            tool_search: String::new(),
            focus_search: false,
            film_search: String::new(),
            film_focus_search: false,
            film_textures: HashMap::new(),
            film_preview_base: None,
            film_preview_params: None,
            film_preview_source: None,
            histogram: None,
            logo,
            wordmark_family,
            status: String::from("Open a folder to start"),
            toast: None,
            dirty: true,
            clipboard: None,
            export_dialog: false,
            crop_rect: None,
            crop_drag: CropDrag::None,
            crop_drag_start: None,
            crop_drag_rect: None,
            crop_aspect: CropAspect::Free,
            crop_orientation: Orientation::Landscape,
            crop_pan: false,
        }
    }

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

    fn open_folder(&mut self) {
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

    fn select_file(&mut self, index: usize) {
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

    fn poll_thumbnails(&mut self) {
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

    fn params_for_index(&self, index: usize) -> PipelineParams {
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

    fn sync_thumbnails(&mut self, ctx: &egui::Context) {
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

    fn sync_texture(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &self.loaded else { return };
        if !self.dirty { return; }

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
                let handle = ui.ctx().load_texture("emulsion-preview", image, egui::TextureOptions::LINEAR);
                self.texture = Some(handle);
            }
        }
        let (base_w, base_h) = (loaded.base_small.width as f32, loaded.base_small.height as f32);
        let size_swapped = self.params.rotate_90 % 2 != 0;

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

    fn current_size(&self) -> Option<(usize, usize)> {
        let index = self.cursor?;
        if let Some(loaded) = &self.loaded {
            return Some(loaded.source_size);
        }
        self.thumb_sizes.get(index).copied().flatten()
    }

    fn current_name(&self) -> String {
        self.cursor
            .and_then(|index| self.files.get(index))
            .map(|path| file_stem(path))
            .unwrap_or_else(|| String::from("No image"))
    }

    fn current_meta(&self) -> String {
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

    fn is_edited_at(&self, index: usize) -> bool {
        self.params_for_index(index) != PipelineParams::default()
    }

    fn is_current_edited(&self) -> bool {
        self.params != PipelineParams::default()
    }

    fn reset_all(&mut self) {
        self.params = PipelineParams::default();
        self.dirty = true;
        self.show_toast("Edits reset");
    }

    fn copy_settings(&mut self) {
        self.clipboard = Some(self.params);
        self.show_toast("Settings copied");
    }

    fn paste_settings(&mut self) {
        if let Some(p) = self.clipboard {
            self.params = p;
            self.crop_rect = p.crop;
            self.crop_drag = CropDrag::None;
            self.crop_drag_start = None;
            self.crop_drag_rect = None;
            self.dirty = true;
            self.thumbs_dirty = true;
            self.show_toast("Settings pasted");
        }
    }

    fn export_modal(&mut self, ui: &mut egui::Ui) {
        if !self.export_dialog {
            return;
        }
        egui::Window::new("Export")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
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
    }

    fn show_toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now() + Duration::from_secs(2)));
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
            .add_filter("PNG", &["png"])
            .add_filter("JPEG", &["jpg"])
            .set_file_name(format!("{}.png", stem))
            .save_file();
        let Some(dest) = choice else {
            self.status = String::from("export cancelled");
            return;
        };
        let s = dest.to_string_lossy().into_owned();
        let lower = s.to_lowercase();
        let result = if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
            emulsion_core::export::save_jpeg(s.as_str(), &out, 92)
        } else {
            emulsion_core::export::save_png(s.as_str(), &out)
        };
        match result {
            Ok(()) => {
                // Sidecar (.emulsion.json) writing is disabled for now.
                // let sidecar = std::path::Path::new(&s)
                //     .with_extension("emulsion.json")
                //     .to_string_lossy()
                //     .into_owned();
                // self.status =
                //     match emulsion_core::export::save_sidecar(&sidecar, &source, &self.params) {
                //         Ok(()) => format!("exported {} + settings", s),
                //         Err(e) => format!("exported {} (settings sidecar failed: {})", s, e.message()),
                //     };
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

    fn top_bar(&mut self, ui: &mut egui::Ui) {
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
                    meta_rect.center().y - meta_galley.size().y / 2.0 + 3.0,
                ),
                meta_galley,
                DIM,
            );

            let export_rect = Rect::from_min_size(
                pos2(rect.right() - 16.0 - 98.0, center_y - 16.0),
                vec2(98.0, 32.0),
            );
            let paste_rect = Rect::from_min_size(
                pos2(export_rect.left() - 8.0 - 66.0, center_y - 16.0),
                vec2(66.0, 32.0),
            );
            let copy_rect = Rect::from_min_size(
                pos2(paste_rect.left() - 8.0 - 66.0, center_y - 16.0),
                vec2(66.0, 32.0),
            );
            let reset_rect = Rect::from_min_size(
                pos2(copy_rect.left() - 8.0 - 66.0, center_y - 16.0),
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

            let small_button = |label: &str, size: Vec2| {
                egui::Button::new(RichText::new(label).size(13.0).color(MUTED))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(8.0)
                    .min_size(size)
            };
            let copy_enabled = self.is_current_edited();
            if ui
                .add_enabled_ui(copy_enabled, |ui| {
                    ui.put(copy_rect, small_button("Copy", copy_rect.size()))
                })
                .inner
                .clicked()
            {
                self.copy_settings();
            }
            let paste_enabled = self.clipboard.is_some();
            if ui
                .add_enabled_ui(paste_enabled, |ui| {
                    ui.put(paste_rect, small_button("Paste", paste_rect.size()))
                })
                .inner
                .clicked()
            {
                self.paste_settings();
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

    fn sidebar(&mut self, ui: &mut egui::Ui) {
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

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Slash)) {
            if self.open_tool == Some(Tool::Simulations) {
                self.film_focus_search = true;
            } else {
                self.focus_search = true;
            }
        }

        let query = self.tool_search.trim().to_lowercase();
        let invert_before = self.params.invert;
        let film_before = self.params.film;
        let mut params = self.params;
        let mut open_tool = self.open_tool;
        let mut search_id = None;

        let frame = egui::Frame::new()
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(8.0)
            .inner_margin(Margin::symmetric(10, 5));
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add_sized(
                    vec2(142.0, 30.0),
                    egui::TextEdit::singleline(&mut self.tool_search)
                        .hint_text(RichText::new("Search tools").color(DIM))
                        .font(FontId::proportional(16.0))
                        .text_color(TEXT)
                        .align(Align2::LEFT_CENTER)
                        .margin(Margin::ZERO)
                        .frame(egui::Frame::NONE),
                );
                search_id = Some(response.id);
                if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.tool_search.clear();
                    response.surrender_focus();
                }

                let (rect, _) = ui.allocate_exact_size(vec2(1.0, 18.0), Sense::hover());
                ui.painter().rect_filled(rect, 0.0, LINE);

                let available_width = ui.available_width();
                egui::ScrollArea::horizontal()
                    .id_salt("tools_horizontal_scroll")
                    .max_width(available_width)
                    .auto_shrink([false, true])
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            const TOOLS: &[Tool] = &[
                                Tool::Develop,
                                Tool::Simulations,
                                Tool::Crop,
                                Tool::Rotate,
                                Tool::Ev,
                                Tool::Contrast,
                                Tool::Shadows,
                                Tool::Highlights,
                                Tool::Wb,
                                Tool::Tint,
                                Tool::Saturation,
                                Tool::Vignette,
                            ];
                            for &tool in TOOLS {
                                if tool_visible(&query, tool) {
                                    let response = tool_tab(
                                        ui,
                                        tool.label(),
                                        open_tool == Some(tool),
                                        tool.changed(&params),
                                    );
                                    if response.clicked() {
                                        open_tool = toggle_tool(open_tool, tool);
                                    }
                                }
                            }
                        })
                    });
            });
        });

        if let Some(id) = search_id
            && self.focus_search
        {
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
            self.focus_search = false;
        }

        if let Some(tool) = open_tool {
            let frame = egui::Frame::new()
                .fill(PANEL_TOP)
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(10.0)
                .inner_margin(Margin::symmetric(15, 0));
            frame.show(ui, |ui| {
                if tool == Tool::Develop {
                    open_tool = None;
                    params.invert = !params.invert;
                } else if tool == Tool::Simulations {
                    self.film_controls(ui, &mut params);
                } else if tool == Tool::Crop {
                    crop_controls(self, ui, &mut params);
                } else {
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), 50.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.allocate_ui_with_layout(
                                vec2(140.0, 50.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.label(
                                        RichText::new(tool.name()).size(14.0).strong().color(TEXT),
                                    );
                                    if tool != Tool::Rotate {
                                        ui.label(
                                            RichText::new(tool.code())
                                                .size(10.5)
                                                .strong()
                                                .color(DIM),
                                        );
                                    }
                                },
                            );

                            if tool == Tool::Rotate {
                                rotate_buttons(ui, &mut params);
                            }

                            match tool {
                                Tool::Crop => {}
                                _ => {
                                    // Standard slider + dragvalue + reset for other tools
                                    let value = tool.value_mut(&mut params).ok_or(0.0).unwrap();
                                    ui.spacing_mut().slider_width = ui.available_width() - 99.0;
                                    let response = ui.add(
                                        egui::Slider::new(value, tool.range())
                                            .show_value(false)
                                            .trailing_fill(true)
                                            .step_by(if tool.decimals() > 0 { 0.1 } else { 1.0 }),
                                    );
                                    let reset = ui.input(|i| {
                                        i.pointer
                                            .button_double_clicked(egui::PointerButton::Primary)
                                            && i.pointer
                                                .interact_pos()
                                                .is_some_and(|pos| response.rect.contains(pos))
                                    });
                                    if reset {
                                        *value = 0.0;
                                    }

                                    let decimals = tool.decimals();
                                    let suffix = tool.suffix();
                                    let mut shown = *value;
                                    ui.allocate_ui_with_layout(
                                        vec2(56.0, 32.0),
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            ui.visuals_mut().override_text_color = Some(ACCENT);
                                            let response = ui.add(
                                                egui::DragValue::new(&mut shown)
                                                    .range(tool.range())
                                                    .speed(if decimals > 0 { 0.1 } else { 1.0 })
                                                    .fixed_decimals(decimals)
                                                    .custom_formatter(move |value, _| {
                                                        signed_value(value, decimals, suffix)
                                                    }),
                                            );
                                            if response.changed() {
                                                *value = shown;
                                            }
                                        },
                                    );

                                    let reset_clicked = ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("↺").size(15.0).color(DIM),
                                            )
                                            .fill(Color32::TRANSPARENT)
                                            .stroke(Stroke::NONE)
                                            .corner_radius(6.0)
                                            .min_size(vec2(28.0, 28.0)),
                                        )
                                        .on_hover_text("Reset adjustment")
                                        .clicked();
                                    if reset_clicked {
                                        *value = 0.0;
                                        if tool == Tool::Rotate {
                                            params.rotate_90 = 0;
                                            params.flip_horizontal = false;
                                        }
                                    }
                                }
                            }
                        },
                    );
                }
            });
        }

        if params != self.params {
            self.params = params;
            self.dirty = true;
            if params.film != film_before {
                match params.film.and_then(film::name) {
                    Some(name) => self.show_toast(format!("Simulations · {}", name)),
                    None => self.show_toast("Simulations · none"),
                }
            } else if params.invert != invert_before {
                self.show_toast(if params.invert {
                    "Develop · inverted"
                } else {
                    "Develop · positive"
                });
            }
        }
        if open_tool != self.open_tool {
            if open_tool == Some(Tool::Crop) {
                // Enter crop: draft starts from the applied crop (may be none).
                self.crop_rect = self.params.crop;
                self.crop_drag = CropDrag::None;
                self.crop_drag_start = None;
                self.crop_drag_rect = None;
                self.crop_pan = false;
                if let Some((w, h)) = self.current_size() {
                    self.crop_orientation = if w >= h {
                        Orientation::Landscape
                    } else {
                        Orientation::Portrait
                    };
                }
                self.dirty = true;
            } else if self.open_tool == Some(Tool::Crop) {
                // Leave crop: apply the draft. A tiny or full-frame rect is "no crop".
                let previous = self.params.crop;
                let applied = self.crop_rect.and_then(normalize_crop);
                self.params.crop = applied;
                self.crop_rect = applied;
                self.crop_drag = CropDrag::None;
                self.crop_drag_start = None;
                self.crop_drag_rect = None;
                self.crop_pan = false;
                self.dirty = true;
                if applied != previous {
                    self.show_toast(if applied.is_some() {
                        "Crop applied"
                    } else {
                        "Crop removed"
                    });
                }
            }
            self.open_tool = open_tool;
        }
    }

    fn film_controls(&mut self, ui: &mut egui::Ui, params: &mut PipelineParams) {
        ui.add_space(10.0);
        let mut search_id = None;
        ui.horizontal(|ui| {
            let selected = params.film.and_then(film::name);
            ui.allocate_ui_with_layout(vec2(164.0, 36.0), Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new("Simulations").size(14.0).strong().color(TEXT));
                let subtitle = ui.add(
                    egui::Label::new(
                        RichText::new(selected.unwrap_or("NO SIMULATION"))
                            .size(10.5)
                            .strong()
                            .color(DIM),
                    )
                    .truncate(),
                );
                if let Some(name) = selected {
                    subtitle.on_hover_text(name);
                }
            });

            ui.add_space(8.0);
            let search_width = (ui.available_width() - 70.0).clamp(180.0, 280.0);
            let frame = egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(255, 252, 245, 180))
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(8.0)
                .inner_margin(Margin::symmetric(10, 0));
            frame.show(ui, |ui| {
                let response = ui.add_sized(
                    vec2(search_width, 28.0),
                    egui::TextEdit::singleline(&mut self.film_search)
                        .hint_text(RichText::new("Search simulations").color(DIM))
                        .font(FontId::proportional(13.0))
                        .text_color(TEXT)
                        .align(Align2::LEFT_CENTER)
                        .margin(Margin::ZERO)
                        .frame(egui::Frame::NONE),
                );
                search_id = Some(response.id);
                if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.film_search.clear();
                    response.surrender_focus();
                }
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let reset = ui
                    .add(
                        egui::Button::new(RichText::new("↺").size(15.0).color(DIM))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .corner_radius(6.0)
                            .min_size(vec2(28.0, 28.0)),
                    )
                    .on_hover_text("Reset film simulation");
                if reset.clicked() {
                    params.film = None;
                }
            });
        });
        if let Some(id) = search_id
            && self.film_focus_search
        {
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
            self.film_focus_search = false;
        }

        ui.add_space(8.0);
        self.film_strip(ui, params);
    }

    fn film_strip(&mut self, ui: &mut egui::Ui, params: &mut PipelineParams) {
        let mut preview_params = *params;
        preview_params.film = None;
        preview_params.crop = None;

        if self.film_preview_source != self.cursor {
            self.film_preview_source = self.cursor;
            self.film_preview_base = None;
            self.film_preview_params = None;
            self.film_textures.clear();
        }
        if self.film_preview_params != Some(preview_params) {
            self.film_preview_params = Some(preview_params);
            self.film_textures.clear();
        }
        if self.film_preview_base.is_none()
            && let Some(loaded) = &self.loaded
        {
            self.film_preview_base =
                Some(emulsion_core::geometry::downscale(&loaded.base_small, 160));
        }

        let query = self.film_search.trim().to_lowercase();
        let ids: Vec<u16> = film::films()
            .iter()
            .enumerate()
            .filter(|(_, film)| query.is_empty() || film.name.to_lowercase().contains(&query))
            .map(|(id, _)| id as u16)
            .collect();

        let preview_base = self.film_preview_base.as_ref();
        let textures = &mut self.film_textures;
        let source = self.cursor;
        let mut chosen = params.film;

        egui::ScrollArea::horizontal()
            .id_salt("film_strip")
            .auto_shrink([false, true])
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .show_viewport(ui, |ui, viewport| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = FILM_CARD_GAP;

                    let x = ui.cursor().min.x;
                    let visible = film_card_visible(x, viewport);
                    let texture = film_preview_texture(
                        textures,
                        preview_base,
                        preview_params,
                        None,
                        source,
                        ui.ctx(),
                        visible,
                    );
                    if film_card(
                        ui,
                        texture.as_ref(),
                        "No simulation",
                        None,
                        chosen.is_none(),
                    ) {
                        chosen = None;
                    }

                    for &id in &ids {
                        let x = ui.cursor().min.x;
                        let visible = film_card_visible(x, viewport);
                        let Some(film) = film::find(id) else { continue };
                        let texture = film_preview_texture(
                            textures,
                            preview_base,
                            preview_params,
                            Some(id),
                            source,
                            ui.ctx(),
                            visible,
                        );
                        if film_card(
                            ui,
                            texture.as_ref(),
                            film.name,
                            Some(film.category),
                            chosen == Some(id),
                        ) {
                            chosen = Some(id);
                        }
                    }
                });
            });

        params.film = chosen;
    }

    fn stage(&mut self, ui: &mut egui::Ui) {
        if ui.input(|i| i.key_pressed(egui::Key::Space)) && !ui.ctx().egui_wants_keyboard_input() {
            self.pan = Vec2::ZERO;
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
            self.pan = Vec2::ZERO;
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
                self.pan = Vec2::ZERO;
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
                if delta != Vec2::ZERO {
                    let next = CropRect {
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

    fn crop_to_screen(&self, cr: CropRect, frame: Rect) -> Rect {
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

    fn crop_center_in_stage(&self, cr: CropRect, frame: Rect) -> Pos2 {
        pos2(
            frame.left() + (cr.x + cr.width * 0.5) * frame.width(),
            frame.top() + (cr.y + cr.height * 0.5) * frame.height(),
        )
    }

    fn handle_crop_interaction(
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

    fn draw_crop_overlay(&self, painter: &egui::Painter, image_rect: Rect, crop: Rect) {
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

    fn draw_crop_frame(&self, painter: &egui::Painter, crop: Rect) {
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

    fn paint_toast(&mut self, ui: &mut egui::Ui) {
        let Some((text, until)) = self.toast.clone() else {
            return;
        };
        let now = Instant::now();
        if now >= until {
            self.toast = None;
            return;
        }
        egui::Area::new(egui::Id::unique("emulsion_toast"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0 + (SIDEBAR_WIDTH / 2.0), -24.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(TOAST_BG)
                    .corner_radius(9.0)
                    .inner_margin(Margin::symmetric(16, 11))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 18],
                        blur: 46,
                        spread: 0,
                        color: Color32::from_black_alpha(90),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
                            ui.painter().circle_filled(
                                rect.center(),
                                3.0,
                                Color32::from_rgb(213, 141, 98),
                            );
                            ui.label(
                                RichText::new(text)
                                    .size(14.0)
                                    .color(Color32::from_rgb(246, 238, 223)),
                            );
                        });
                    });
            });
        ui.ctx().request_repaint_after(until - now);
    }
}

impl eframe::App for EmulsionApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_load();
        self.poll_thumbnails();
        if self.thumb_rx.is_some() || self.load_rx.is_some() {
            ui.ctx().request_repaint();
        }
        if self.dirty {
            self.sync_texture(ui);
        }
        if self.thumbs_dirty {
            self.sync_thumbnails(ui.ctx());
        }

        ui.style_mut().always_scroll_the_only_direction = true;

        egui::Panel::top("top_panel")
            .exact_size(TOPBAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(247, 241, 230))
                    .inner_margin(Margin::symmetric(16, 0)),
            )
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::left("file_panel")
            .exact_size(SIDEBAR_WIDTH)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(13, 16)),
            )
            .show(ui, |ui| self.sidebar(ui));

        let settings_height = match self.open_tool {
            Some(Tool::Simulations) => SETTINGS_SIMULATIONS_HEIGHT,
            Some(_) => SETTINGS_OPEN_HEIGHT,
            None => SETTINGS_HEIGHT,
        };
        egui::Panel::top("settings_panel")
            .exact_size(settings_height)
            .frame(
                egui::Frame::new()
                    .fill(SETTINGS_BG)
                    .inner_margin(Margin::symmetric(16, 11)),
            )
            .show(ui, |ui| self.settings_panel(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(STAGE_OUT)
                    .inner_margin(Margin::ZERO),
            )
            .show(ui, |ui| self.stage(ui));

        self.paint_toast(ui);
        self.export_modal(ui);
    }
}

fn install_fonts(ctx: &egui::Context) -> FontFamily {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "emulsion-ui".to_owned(),
        Arc::new(egui::FontData::from_owned(
            include_bytes!("../assets/fonts/Ubuntu-Regular.ttf").to_vec(),
        )),
    );
    fonts.font_data.insert(
        "emulsion-wordmark".to_owned(),
        Arc::new(egui::FontData::from_owned(
            include_bytes!("../assets/fonts/Ubuntu-Medium.ttf").to_vec(),
        )),
    );
    fonts.font_data.insert(
        "emulsion-mono".to_owned(),
        Arc::new(egui::FontData::from_owned(
            include_bytes!("../assets/fonts/UbuntuMono-Regular.ttf").to_vec(),
        )),
    );
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "emulsion-ui".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.insert(0, "emulsion-mono".to_owned());
    }

    let family = FontFamily::Name("emulsion-serif".into());
    fonts.families.insert(
        family.clone(),
        vec!["emulsion-wordmark".to_owned(), "emulsion-ui".to_owned()],
    );

    ctx.set_fonts(fonts);
    family
}

fn install_style(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = PANEL_TOP;
    style.visuals.faint_bg_color = SETTINGS_BG;
    style.visuals.override_text_color = None;
    style.visuals.weak_text_color = Some(DIM);
    style.visuals.selection.bg_fill = ACCENT_SOFT;
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.widgets.noninteractive.bg_fill = PANEL;
    style.visuals.widgets.noninteractive.weak_bg_fill = PANEL;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(233, 225, 209);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, MUTED);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(225, 216, 199);
    style.visuals.widgets.hovered.weak_bg_fill = HOVER;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, LINE_STRONG);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(216, 207, 189);
    style.visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_LINE);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.open.bg_fill = ACCENT_SOFT;
    style.visuals.widgets.open.weak_bg_fill = ACCENT_SOFT;
    style.visuals.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_LINE);
    style.visuals.widgets.open.fg_stroke = Stroke::new(1.0, ACCENT);
    style.spacing.item_spacing = vec2(6.0, 6.0);
    style.spacing.button_padding = vec2(10.0, 6.0);
    style.spacing.interact_size = vec2(32.0, 28.0);
    style.spacing.slider_rail_height = 3.0;
    style.spacing.slider_width = 180.0;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(11.5));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(12.0));
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
}

fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(include_bytes!("../assets/logo.png")).ok()?;
    let image = image.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture("emulsion-logo", color, egui::TextureOptions::LINEAR))
}

fn file_name(path: &str) -> String {
    match std::path::Path::new(path).file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => String::from(path),
    }
}

fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name(path))
}

fn file_extension(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .map(|s| s.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| String::from("IMAGE"))
}

fn film_card_visible(x: f32, viewport: Rect) -> bool {
    x + FILM_CARD_WIDTH >= viewport.left() - FILM_PRELOAD_MARGIN
        && x <= viewport.right() + FILM_PRELOAD_MARGIN
}

fn film_preview_texture(
    textures: &mut HashMap<Option<u16>, egui::TextureHandle>,
    base: Option<&emulsion_core::image::LinearImage>,
    preview_params: PipelineParams,
    film_id: Option<u16>,
    source: Option<usize>,
    ctx: &egui::Context,
    visible: bool,
) -> Option<egui::TextureHandle> {
    if !visible {
        return textures.get(&film_id).cloned();
    }
    if let Some(texture) = textures.get(&film_id) {
        return Some(texture.clone());
    }
    let base = base?;
    let mut params = preview_params;
    params.film = film_id;
    let out = develop(base, &params);
    let image = egui::ColorImage::from_rgba_unmultiplied([out.width, out.height], &out.data);
    let name = format!("film-preview-{:?}-{:?}", source, film_id);
    let texture = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
    textures.insert(film_id, texture.clone());
    Some(texture)
}

fn film_card(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    name: &str,
    category: Option<film::Category>,
    selected: bool,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(vec2(FILM_CARD_WIDTH, FILM_CARD_HEIGHT), Sense::click());
    let painter = ui.painter();
    let inner = rect.shrink(1.0);
    let hovered = response.hovered();

    if selected {
        // painter.rect_filled(inner, 8.0, ACCENT_SOFT);
    } else if hovered {
        // painter.rect_filled(inner, 8.0, HOVER);
    }

    let thumb = Rect::from_min_size(
        pos2(inner.left() + 4.0, inner.top() + 4.0),
        vec2(inner.width() - 8.0, FILM_THUMB_HEIGHT),
    );
    painter.rect_filled(thumb, 5.0, Color32::from_rgb(233, 225, 209));
    match texture {
        Some(texture) => {
            let uv = cover_uv(texture.size_vec2(), thumb.size());
            egui::Image::from_texture(texture)
                .uv(uv)
                .corner_radius(5.0)
                .maintain_aspect_ratio(false)
                .fit_to_exact_size(thumb.size())
                .paint_at(ui, thumb);
        }
        None => {
            painter.text(
                thumb.center(),
                Align2::CENTER_CENTER,
                "…",
                FontId::proportional(16.0),
                DIM,
            );
        }
    }
    painter.rect_stroke(
        thumb,
        5.0,
        if selected || hovered {
            Stroke::new(2.0, ACCENT)
        } else {
            Stroke::new(1.0, LINE)
        },
        StrokeKind::Inside,
    );

    if let Some(category) = category {
        let label = match category {
            film::Category::BlackAndWhite => "B&W",
            film::Category::Color => "COLOR",
        };
        let galley =
            painter.layout_no_wrap(label.to_owned(), FontId::monospace(8.0), Color32::WHITE);
        let badge = Rect::from_min_size(
            pos2(thumb.left() + 4.0, thumb.top() + 4.0),
            galley.size() + vec2(8.0, 4.0),
        );
        painter.rect_filled(badge, 3.0, Color32::from_black_alpha(130));
        painter.galley(badge.min + vec2(4.0, 2.0), galley, Color32::WHITE);
    }

    let text_rect = Rect::from_min_max(
        pos2(inner.left() + 3.0, thumb.bottom() + 4.0),
        pos2(inner.right() - 3.0, inner.bottom() - 2.0),
    );
    let color = if selected { TEXT } else { MUTED };
    let galley = painter.layout(
        name.to_owned(),
        FontId::proportional(10.5),
        color,
        text_rect.width(),
    );
    painter
        .with_clip_rect(text_rect)
        .galley(text_rect.min, galley, color);

    response.clicked()
}

fn rotate_buttons(ui: &mut egui::Ui, params: &mut PipelineParams) {
    let btn = |label: &str| {
        egui::Button::new(RichText::new(label).size(12.0).color(MUTED))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(6.0)
            .min_size(vec2(52.0, 28.0))
    };
    if ui.add(btn("90L")).clicked() {
        params.rotate_90 = (params.rotate_90 + 3) % 4;
    }
    if ui.add(btn("90R")).clicked() {
        params.rotate_90 = (params.rotate_90 + 1) % 4;
    }
    let flip = egui::Button::new(RichText::new("FLIP").size(12.0).color(
        if params.flip_horizontal {
            ACCENT
        } else {
            MUTED
        },
    ))
    .fill(if params.flip_horizontal {
        ACCENT_SOFT
    } else {
        Color32::TRANSPARENT
    })
    .stroke(Stroke::new(
        1.0,
        if params.flip_horizontal {
            ACCENT_LINE
        } else {
            LINE
        },
    ))
    .corner_radius(6.0)
    .min_size(vec2(52.0, 28.0));
    if ui.add(flip).clicked() {
        params.flip_horizontal = !params.flip_horizontal;
    }
    ui.add_space(4.0);
}

fn crop_controls(app: &mut EmulsionApp, ui: &mut egui::Ui, params: &mut PipelineParams) {
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), 50.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.allocate_ui_with_layout(
                vec2(140.0, 50.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.label(RichText::new("Crop").size(14.0).strong().color(TEXT));
                },
            );
            ui.add_space(18.0);

            let mut aspect = app.crop_aspect;
            egui::ComboBox::from_id_salt("crop_aspect")
                .selected_text(aspect.label())
                .width(92.0)
                .show_ui(ui, |ui| {
                    for option in CropAspect::ALL {
                        ui.selectable_value(&mut aspect, option, option.label());
                    }
                });
            if aspect != app.crop_aspect {
                app.crop_aspect = aspect;
                if !aspect.is_free() {
                    app.reshape_crop(params);
                }
            }

            ui.add_space(12.0);
            let mut orientation = app.crop_orientation;
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
            if orientation != app.crop_orientation {
                app.crop_orientation = orientation;
                if !app.crop_aspect.is_free() {
                    app.reshape_crop(params);
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
                    app.crop_rect = None;
                    app.crop_drag = CropDrag::None;
                    app.crop_drag_start = None;
                    app.crop_drag_rect = None;
                }
            });
        },
    );
}

fn normalize_crop(cr: CropRect) -> Option<CropRect> {
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

fn tool_tab(ui: &mut egui::Ui, label: &str, active: bool, changed: bool) -> egui::Response {
    let font = FontId::proportional(12.0);
    let text_width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), MUTED)
        .size()
        .x;
    let (rect, response) = ui.allocate_exact_size(vec2(text_width + 26.0, 32.0), Sense::click());
    let hovered = response.hovered();
    let color = if active {
        ACCENT
    } else if hovered {
        TEXT
    } else if changed {
        Color32::from_rgb(77, 67, 50)
    } else {
        MUTED
    };
    let fill = if active {
        ACCENT_SOFT
    } else if hovered {
        HOVER
    } else {
        Color32::TRANSPARENT
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 7.0, fill);
    if active {
        painter.rect_stroke(rect, 7.0, Stroke::new(1.0, ACCENT_LINE), StrokeKind::Inside);
    }
    painter.text(rect.center(), Align2::CENTER_CENTER, label, font, color);
    if changed && !active {
        painter.circle_filled(pos2(rect.right() - 7.0, rect.top() + 8.0), 2.0, ACCENT);
    }
    response
}

fn tool_visible(query: &str, tool: Tool) -> bool {
    query.is_empty()
        || tool.label().to_lowercase().contains(query)
        || tool.name().to_lowercase().contains(query)
        || tool.aliases().contains(query)
}

fn toggle_tool(open: Option<Tool>, tool: Tool) -> Option<Tool> {
    if open == Some(tool) { None } else { Some(tool) }
}

fn signed_value(value: f64, decimals: usize, suffix: &str) -> String {
    let sign = if value > 0.0 { "+" } else { "" };
    format!("{}{:.*}{}", sign, decimals, value, suffix)
}

fn make_histogram(data: &[u8]) -> Option<Histogram> {
    if data.is_empty() {
        return None;
    }
    let mut counts = [[0_u32; 64]; 3];
    for pixel in data.as_chunks::<4>().0 {
        for channel in 0..3 {
            let bin = (pixel[channel] as usize * 63) / 255;
            counts[channel][bin] += 1;
        }
    }
    let max = counts
        .iter()
        .flat_map(|channel| channel.iter())
        .copied()
        .max()
        .unwrap_or(0);
    if max == 0 {
        return None;
    }
    let mut channels = [[0.0_f32; 64]; 3];
    for channel in 0..3 {
        for bin in 0..64 {
            channels[channel][bin] = (counts[channel][bin] as f32 / max as f32).sqrt();
        }
    }
    Some(Histogram { channels })
}

fn paint_histogram(ui: &mut egui::Ui, histogram: Option<&Histogram>, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let painter = ui.painter();
    for i in 1..3 {
        let y = rect.top() + rect.height() * i as f32 / 3.0;
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0, LINE),
        );
    }
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, LINE),
    );

    let Some(histogram) = histogram else {
        return;
    };
    let colors = [
        Color32::from_rgb(201, 121, 99),
        Color32::from_rgb(120, 163, 132),
        Color32::from_rgb(120, 148, 189),
    ];
    let bin_width = rect.width() / 64.0;
    for (channel, color) in colors.iter().enumerate() {
        let color = *color;
        for (bin, value) in histogram.channels[channel].iter().enumerate() {
            let x = rect.left() + bin as f32 * bin_width;
            let h = value.clamp(0.0, 1.0) * (rect.height() - 2.0);
            let bar = Rect::from_min_max(
                pos2(x, rect.bottom() - h),
                pos2(x + bin_width.max(1.0), rect.bottom()),
            );
            painter.rect_filled(
                bar,
                0.0,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 18),
            );
            painter.line_segment(
                [
                    pos2(x, rect.bottom() - h),
                    pos2(x + bin_width.max(1.0), rect.bottom() - h),
                ],
                Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120),
                ),
            );
        }
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

fn aspect_label(w: usize, h: usize) -> String {
    if h == 0 {
        return String::from("—");
    }
    let ratio = w as f32 / h as f32;
    for (rw, rh, label) in [
        (3.0, 2.0, "3:2"),
        (4.0, 3.0, "4:3"),
        (16.0, 9.0, "16:9"),
        (1.0, 1.0, "1:1"),
        (4.0, 5.0, "4:5"),
        (2.0, 3.0, "2:3"),
        (5.0, 4.0, "5:4"),
    ] {
        if ((ratio - rw / rh).abs() / ratio) < 0.02 {
            return String::from(label);
        }
    }
    format!("{:.2}", ratio)
}

fn cover_uv(image: Vec2, target: Vec2) -> Rect {
    if image.x <= 0.0 || image.y <= 0.0 || target.x <= 0.0 || target.y <= 0.0 {
        return Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
    }
    let image_aspect = image.x / image.y;
    let target_aspect = target.x / target.y;
    if image_aspect > target_aspect {
        let fraction = target_aspect / image_aspect;
        let inset = (1.0 - fraction) / 2.0;
        Rect::from_min_max(pos2(inset, 0.0), pos2(1.0 - inset, 1.0))
    } else {
        let fraction = image_aspect / target_aspect;
        let inset = (1.0 - fraction) / 2.0;
        Rect::from_min_max(pos2(0.0, inset), pos2(1.0, 1.0 - inset))
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

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([1000.0, 680.0]),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Emulsion",
        options,
        Box::new(|cc| Ok(Box::new(EmulsionApp::new(cc)))),
    );
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn film_card_visibility_preloads_two_cards() {
        let viewport = Rect::from_min_size(pos2(100.0, 0.0), vec2(300.0, 100.0));
        assert!(film_card_visible(96.0, viewport));
        assert!(film_card_visible(420.0, viewport));
        assert!(film_card_visible(400.0 + FILM_PRELOAD_MARGIN, viewport));
        assert!(!film_card_visible(401.0 + FILM_PRELOAD_MARGIN, viewport));
        assert!(film_card_visible(-204.0, viewport));
        assert!(!film_card_visible(-205.0, viewport));
    }

    #[test]
    fn film_panel_widgets_render_headless() {
        egui::__run_test_ui(|ui| {
            let mut base = emulsion_core::image::LinearImage::new(2, 2);
            base.set_px(0, 0, 0.2, 0.5, 0.8);
            let mut textures = HashMap::new();
            let texture = film_preview_texture(
                &mut textures,
                Some(&base),
                PipelineParams::default(),
                Some(0),
                None,
                ui.ctx(),
                true,
            );
            assert!(texture.is_some(), "preview texture was not built");
            assert!(textures.contains_key(&Some(0)));
            let clicked = film_card(
                ui,
                texture.as_ref(),
                "Agfa APX 100",
                Some(film::Category::BlackAndWhite),
                false,
            );
            assert!(!clicked);
            let clicked = film_card(ui, None, "No simulation", None, true);
            assert!(!clicked);
        });
    }
}
