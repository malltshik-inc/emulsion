#![windows_subsystem = "windows"]

use eframe::egui;
use egui::{
    Align, Align2, Color32, FontFamily, FontId, Layout, Margin, Pos2, Rect, RichText, Sense,
    Stroke, StrokeKind, TextStyle, Vec2, pos2, vec2,
};

use emulsion_core::{
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
const SETTINGS_HEIGHT: f32 = 58.0;
const SETTINGS_OPEN_HEIGHT: f32 = 118.0;

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
    source_size: (usize, usize),
}

#[derive(Clone)]
struct Histogram {
    channels: [[f32; 64]; 3],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Wb,
    Tint,
    Ev,
    Contrast,
    Saturation,
    Rotate,
    Crop,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Wb => "White Balance",
            Self::Tint => "Tint",
            Self::Ev => "Exposure",
            Self::Contrast => "Contrast",
            Self::Saturation => "Saturation",
            Self::Rotate => "Rotate",
            Self::Crop => "Crop",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Wb => "WB",
            Self::Tint => "TINT",
            Self::Ev => "EV",
            Self::Contrast => "CONTRAST",
            Self::Saturation => "SATURATION",
            Self::Rotate => "ROTATE",
            Self::Crop => "CROP",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Wb => "WB",
            Self::Tint => "TINT",
            Self::Ev => "EV",
            Self::Contrast => "CONTRAST",
            Self::Saturation => "SATURATION",
            Self::Rotate => "ANGLE",
            Self::Crop => "ASPECT",
        }
    }

    fn aliases(self) -> &'static str {
        match self {
            Self::Wb => "white balance temperature warm cool",
            Self::Tint => "tint magenta green",
            Self::Ev => "exposure brightness",
            Self::Contrast => "contrast",
            Self::Saturation => "saturation vibrance color",
            Self::Rotate => "rotate angle straighten",
            Self::Crop => "crop aspect ratio",
        }
    }

    fn range(self) -> std::ops::RangeInclusive<f32> {
        match self {
            Self::Ev => -3.0..=3.0,
            Self::Rotate => -5.0..=5.0,
            Self::Crop => 0.0..=0.0, // Crop uses different UI
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

    fn value_mut(self, params: &mut PipelineParams) -> &mut f32 {
        match self {
            Self::Wb => &mut params.wb_temperature,
            Self::Tint => &mut params.wb_tint,
            Self::Ev => &mut params.exposure,
            Self::Contrast => &mut params.contrast,
            Self::Saturation => &mut params.saturation,
            Self::Rotate => &mut params.fine_angle,
            Self::Crop => {
                // Crop doesn't have a single f32 value; this should not be called for Crop
                fn get_dummy() -> &'static mut f32 {
                    static mut DUMMY: f32 = 0.0;
                    let ptr: *mut f32 = &raw mut DUMMY;
                    unsafe { &mut *ptr }
                }
                get_dummy()
            }
        }
    }

    fn changed(self, params: &PipelineParams) -> bool {
        match self {
            Self::Rotate => params.fine_angle != 0.0 || params.rotate_90 != 0,
            Self::Crop => params.crop.is_some(),
            _ => {
                let mut copy = *params;
                *self.value_mut(&mut copy) != 0.0
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CropAspect {
    Original,
    Square,
    FourThree,
    ThreeTwo,
    SixteenNine,
    TwoOne,
}

impl CropAspect {
    const ALL: [CropAspect; 6] = [
        Self::Original,
        Self::Square,
        Self::FourThree,
        Self::ThreeTwo,
        Self::SixteenNine,
        Self::TwoOne,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Original => "Original",
            Self::Square => "1:1",
            Self::FourThree => "4:3",
            Self::ThreeTwo => "3:2",
            Self::SixteenNine => "16:9",
            Self::TwoOne => "2:1",
        }
    }

    /// Base width / height, or `None` for a free (unconstrained) crop.
    fn ratio(self) -> Option<f32> {
        match self {
            Self::Original => None,
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
    histogram: Option<Histogram>,

    logo: Option<egui::TextureHandle>,
    wordmark_family: FontFamily,
    status: String,
    toast: Option<(String, Instant)>,
    dirty: bool,

    // Crop state
    crop_rect: Option<CropRect>,      // live crop (mirrors params.crop)
    crop_drag: CropDrag,              // current drag state
    crop_drag_start: Option<Pos2>,    // drag start position
    crop_drag_rect: Option<CropRect>, // rect at drag start
    crop_aspect: CropAspect,          // aspect-ratio constraint
    crop_orientation: Orientation,    // portrait / landscape
    crop_backup: Option<CropRect>,    // crop to restore on Cancel
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
            histogram: None,
            logo,
            wordmark_family,
            status: String::from("Open a folder to start"),
            toast: None,
            dirty: true,
            crop_rect: None,
            crop_drag: CropDrag::None,
            crop_drag_start: None,
            crop_drag_rect: None,
            crop_aspect: CropAspect::Original,
            crop_orientation: Orientation::Landscape,
            crop_backup: None,
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
        self.crop_backup = self.params.crop;
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
            let base_small = emulsion_core::geometry::downscale(&base, 1800);
            let _ = sender.send(Ok(Loaded {
                base_small,
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
        }
        self.thumbs_dirty = false;
    }

    fn sync_texture(&mut self, ui: &mut egui::Ui) {
        let Some(loaded) = &self.loaded else { return };
        if !self.dirty {
            return;
        }
        // Preview always shows the full frame; crop is an overlay applied on export.
        let mut preview_params = self.params;
        preview_params.crop = None;
        let out = develop(&loaded.base_small, &preview_params);
        let w = out.width as u32;
        let h = out.height as u32;
        self.histogram = make_histogram(&out.data);
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
        self.thumbs_dirty = true;
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
                let sidecar = std::path::Path::new(&s)
                    .with_extension("emulsion.json")
                    .to_string_lossy()
                    .into_owned();
                self.status =
                    match emulsion_core::export::save_sidecar(&sidecar, &source, &self.params) {
                        Ok(()) => format!("exported {} + settings", s),
                        Err(e) => {
                            format!("exported {} (settings sidecar failed: {})", s, e.message())
                        }
                    };
                self.show_toast(self.status.clone());
            }
            Err(e) => {
                self.status = format!("export error: {}", e.message());
                self.show_toast(self.status.clone());
            }
        }
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

        let word_y = center_y + 2.0;
        let word_font = FontId::new(21.0, self.wordmark_family.clone());
        let word_rect = painter.text(
            pos2(logo_rect.right() + 10.0, word_y),
            Align2::LEFT_CENTER,
            "Emulsion",
            word_font.clone(),
            TEXT,
        );
        painter.text(
            pos2(word_rect.right() + 1.0, word_y),
            Align2::LEFT_CENTER,
            ".",
            word_font,
            ACCENT,
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
                    meta_rect.center().y - meta_galley.size().y / 2.0 + 1.0,
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
                self.export();
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
            self.focus_search = true;
        }

        let query = self.tool_search.trim().to_lowercase();
        let invert_before = self.params.invert;
        let mut params = self.params;
        let mut open_tool = self.open_tool;
        let mut search_id = None;

        ui.horizontal(|ui| {
            let frame = egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(255, 252, 245, 180))
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(8.0)
                .inner_margin(Margin::symmetric(10, 0));
            frame.show(ui, |ui| {
                let response = ui.add_sized(
                    vec2(142.0, 30.0),
                    egui::TextEdit::singleline(&mut self.tool_search)
                        .hint_text(RichText::new("Search tools").color(DIM))
                        .font(FontId::proportional(14.0))
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
            });
            ui.add_space(10.0);

            let separator_visible = query.is_empty();
            let film_visible =
                query.is_empty() || "film negative positive invert develop".contains(&query);
            if film_visible {
                let response = tool_tab(ui, "FILM", params.invert, false);
                if response.clicked() {
                    params.invert = !params.invert;
                }
            }
            const TOOLS: &[Tool] = &[
                Tool::Crop,
                Tool::Rotate,
                Tool::Wb,
                Tool::Tint,
                Tool::Ev,
                Tool::Contrast,
                Tool::Saturation,
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

            if separator_visible {
                let (rect, _) = ui.allocate_exact_size(vec2(1.0, 18.0), Sense::hover());
                ui.painter().rect_filled(rect, 0.0, LINE);
            }
        });

        if let Some(id) = search_id
            && self.focus_search
        {
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
            self.focus_search = false;
        }

        if let Some(tool) = open_tool {
            ui.add_space(10.0);
            let frame = egui::Frame::new()
                .fill(PANEL_TOP)
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(10.0)
                .inner_margin(Margin::symmetric(15, 0));
            frame.show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), 50.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        if tool == Tool::Crop {
                            ui.allocate_ui_with_layout(
                                vec2(140.0, 50.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    ui.label(RichText::new("Crop").size(14.0).strong().color(TEXT));
                                },
                            );
                        } else {
                            ui.allocate_ui_with_layout(
                                vec2(164.0, 36.0),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.label(
                                        RichText::new(tool.name()).size(14.0).strong().color(TEXT),
                                    );
                                    ui.label(
                                        RichText::new(tool.code()).size(10.5).strong().color(DIM),
                                    );
                                },
                            );
                        }

                        if tool == Tool::Rotate {
                            rotate_buttons(ui, &mut params);
                        }

                        match tool {
                            Tool::Crop => {
                                ui.add_space(4.0);
                                if crop_controls(self, ui, &mut params) {
                                    open_tool = None;
                                }
                            }
                            _ => {
                                // Standard slider + dragvalue + reset for other tools
                                let value = tool.value_mut(&mut params);
                                ui.spacing_mut().slider_width =
                                    (ui.available_width() - 56.0 - 28.0 - 32.0).max(70.0);
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
                                        egui::Button::new(RichText::new("↺").size(15.0).color(DIM))
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
                                    }
                                }
                            }
                        }
                    },
                );
            });
        }

        if params != self.params {
            self.params = params;
            self.dirty = true;
            if params.invert != invert_before {
                self.show_toast(if params.invert {
                    "Film · inverted"
                } else {
                    "Film · positive"
                });
            }
        }
        if open_tool != self.open_tool {
            if open_tool == Some(Tool::Crop) {
                self.crop_backup = self.params.crop;
                if let Some((w, h)) = self.current_size() {
                    self.crop_orientation = if w >= h {
                        Orientation::Landscape
                    } else {
                        Orientation::Portrait
                    };
                }
                if self.crop_rect.is_none() {
                    self.crop_rect = Some(CropRect {
                        x: 0.1,
                        y: 0.1,
                        width: 0.8,
                        height: 0.8,
                    });
                }
            } else if self.open_tool == Some(Tool::Crop) {
                // Leaving Crop without Apply: discard an uncommitted draft.
                self.crop_rect = self.params.crop;
            }
            self.open_tool = open_tool;
        }
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
        let crop_screen = self.crop_rect.map(|cr| self.crop_to_screen(cr, frame_rect));

        // Handle crop interaction (needs mutable self)
        self.handle_crop_interaction(ui, &response, frame_rect, crop_screen);

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

        // Draw crop overlay (darken outside) and crop frame
        if let Some(cr) = crop_screen {
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
        if response.is_pointer_button_down_on() {
            let delta = ui.input(|i| i.pointer.delta());
            self.pan += delta;
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
        crop_screen: Option<Rect>,
    ) {
        let mut crop = match self.crop_rect {
            Some(cr) => cr,
            None => return,
        };

        let Some(_crop_rect) = crop_screen else {
            return;
        };
        let pointer = ui.input(|i| i.pointer.interact_pos());

        // Handle drag
        if self.crop_drag != CropDrag::None {
            if let Some(pos) = pointer {
                if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                    let drag_start = self.crop_drag_start.unwrap_or(pos);
                    let start_rect = self.crop_drag_rect.unwrap_or(crop);
                    let delta = pos - drag_start;

                    // Convert screen delta to normalized crop delta
                    let norm_delta = vec2(delta.x / frame.width(), delta.y / frame.height());

                    match self.crop_drag {
                        CropDrag::Rect => {
                            crop.x =
                                (start_rect.x + norm_delta.x).clamp(0.0, 1.0 - start_rect.width);
                            crop.y =
                                (start_rect.y + norm_delta.y).clamp(0.0, 1.0 - start_rect.height);
                        }
                        CropDrag::TL => {
                            crop.x = (start_rect.x + norm_delta.x)
                                .clamp(0.0, start_rect.x + start_rect.width - 0.01);
                            crop.y = (start_rect.y + norm_delta.y)
                                .clamp(0.0, start_rect.y + start_rect.height - 0.01);
                            crop.width = (start_rect.width - norm_delta.x).max(0.01);
                            crop.height = (start_rect.height - norm_delta.y).max(0.01);
                        }
                        CropDrag::TR => {
                            crop.y = (start_rect.y + norm_delta.y)
                                .clamp(0.0, start_rect.y + start_rect.height - 0.01);
                            crop.width = (start_rect.width + norm_delta.x)
                                .max(0.01)
                                .min(1.0 - start_rect.x);
                            crop.height = (start_rect.height - norm_delta.y).max(0.01);
                        }
                        CropDrag::BL => {
                            crop.x = (start_rect.x + norm_delta.x)
                                .clamp(0.0, start_rect.x + start_rect.width - 0.01);
                            crop.width = (start_rect.width - norm_delta.x).max(0.01);
                            crop.height = (start_rect.height + norm_delta.y)
                                .max(0.01)
                                .min(1.0 - start_rect.y);
                        }
                        CropDrag::BR => {
                            crop.width = (start_rect.width + norm_delta.x)
                                .max(0.01)
                                .min(1.0 - start_rect.x);
                            crop.height = (start_rect.height + norm_delta.y)
                                .max(0.01)
                                .min(1.0 - start_rect.y);
                        }
                        CropDrag::T => {
                            crop.y = (start_rect.y + norm_delta.y)
                                .clamp(0.0, start_rect.y + start_rect.height - 0.01);
                            crop.height = (start_rect.height - norm_delta.y).max(0.01);
                        }
                        CropDrag::B => {
                            crop.height = (start_rect.height + norm_delta.y)
                                .max(0.01)
                                .min(1.0 - start_rect.y);
                        }
                        CropDrag::L => {
                            crop.x = (start_rect.x + norm_delta.x)
                                .clamp(0.0, start_rect.x + start_rect.width - 0.01);
                            crop.width = (start_rect.width - norm_delta.x).max(0.01);
                        }
                        CropDrag::R => {
                            crop.width = (start_rect.width + norm_delta.x)
                                .max(0.01)
                                .min(1.0 - start_rect.x);
                        }
                        CropDrag::None => {}
                    }

                    // Dragging is free-form; aspect presets reshape on selection.
                    self.crop_rect = Some(crop);
                    self.params.crop = Some(crop);
                    self.dirty = true;
                } else {
                    // Mouse released
                    self.crop_drag = CropDrag::None;
                    self.crop_drag_start = None;
                    self.crop_drag_rect = None;
                }
            } else {
                // Not dragging - check hover for handle detection
                if let Some(pos) = pointer {
                    let handle = self.crop_handle_at(pos, crop, frame);
                    if handle != CropDrag::None {
                        response.ctx.set_cursor_icon(match handle {
                            CropDrag::TL | CropDrag::BR => egui::CursorIcon::ResizeNwSe,
                            CropDrag::TR | CropDrag::BL => egui::CursorIcon::ResizeNeSw,
                            CropDrag::T | CropDrag::B => egui::CursorIcon::ResizeVertical,
                            CropDrag::L | CropDrag::R => egui::CursorIcon::ResizeHorizontal,
                            CropDrag::Rect => egui::CursorIcon::Move,
                            CropDrag::None => egui::CursorIcon::Default,
                        });
                    }

                    // Start drag on click
                    if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                        let handle = self.crop_handle_at(pos, crop, frame);
                        if handle != CropDrag::None {
                            self.crop_drag = handle;
                            self.crop_drag_start = Some(pos);
                            self.crop_drag_rect = Some(crop);
                        }
                    }
                }
            }
        }
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
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -24.0))
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

        let settings_height = if self.open_tool.is_some() {
            SETTINGS_OPEN_HEIGHT
        } else {
            SETTINGS_HEIGHT
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
    }
}

fn install_fonts(ctx: &egui::Context) -> FontFamily {
    let mut fonts = egui::FontDefinitions::default();

    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
        fonts.font_data.insert(
            "emulsion-ui".to_owned(),
            Arc::new(egui::FontData::from_owned(bytes)),
        );
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, "emulsion-ui".to_owned());
        }
    }
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\consola.ttf") {
        fonts.font_data.insert(
            "emulsion-mono".to_owned(),
            Arc::new(egui::FontData::from_owned(bytes)),
        );
        if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
            family.insert(0, "emulsion-mono".to_owned());
        }
    }

    let mut serif = Vec::new();
    for (key, path) in [
        ("emulsion-serif", r"C:\Windows\Fonts\pala.ttf"),
        ("emulsion-serif-alt", r"C:\Windows\Fonts\georgia.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(key.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
            serif.push(key.to_owned());
        }
    }

    let family = if serif.is_empty() {
        FontFamily::Proportional
    } else {
        let family = FontFamily::Name("emulsion-serif".into());
        fonts.families.insert(family.clone(), serif);
        family
    };

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
    ui.add_space(4.0);
}

fn crop_controls(app: &mut EmulsionApp, ui: &mut egui::Ui, params: &mut PipelineParams) -> bool {
    let mut close = false;
    let apply = egui::Button::new(RichText::new("✓ Apply").size(12.5).color(Color32::WHITE))
        .fill(ACCENT)
        .stroke(Stroke::NONE)
        .corner_radius(6.0)
        .min_size(vec2(84.0, 30.0));
    if ui.add(apply).clicked() {
        params.crop = app.crop_rect;
        app.crop_backup = app.crop_rect;
        app.show_toast("Crop applied");
        close = true;
    }

    let cancel = egui::Button::new(RichText::new("✕ Cancel").size(12.5).color(MUTED))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(6.0)
        .min_size(vec2(84.0, 30.0));
    if ui.add(cancel).clicked() {
        params.crop = app.crop_backup;
        app.crop_rect = app.crop_backup;
        app.crop_drag = CropDrag::None;
        close = true;
    }

    ui.add_space(14.0);
    ui.label(RichText::new("ASPECT RATIO").size(10.5).strong().color(DIM));
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
        app.reshape_crop(params);
    }

    ui.add_space(12.0);
    ui.label(RichText::new("ORIENTATION").size(10.5).strong().color(DIM));
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
        app.reshape_crop(params);
    }

    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let reset = ui
            .add(
                egui::Button::new(RichText::new("↺").size(16.0).color(DIM))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE)
                    .corner_radius(6.0)
                    .min_size(vec2(30.0, 30.0)),
            )
            .on_hover_text("Reset crop");
        if reset.clicked() {
            params.crop = None;
            app.crop_rect = None;
            app.crop_backup = None;
            app.crop_drag = CropDrag::None;
            app.crop_drag_start = None;
            app.crop_drag_rect = None;
        }
    });
    close
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
