//! Application state, constructor and eframe frame loop.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use eframe::egui;
use egui::{FontFamily, Pos2, Vec2};
use emulsion_core::params::{CropRect, PipelineParams};

use crate::histogram::Histogram;
use crate::model::{CropAspect, CropDrag, Loaded, Orientation, ThumbMessage, Tool, TreeNode};
use crate::theme::{OPEN_FOLDER_HINT, install_fonts, install_style, load_logo};

pub(crate) struct EmulsionApp {
    pub(crate) files: Vec<String>,
    pub(crate) tree: Vec<TreeNode>,
    pub(crate) expanded: HashSet<String>,
    pub(crate) cursor: Option<usize>,
    pub(crate) loaded: Option<Loaded>,

    pub(crate) load_rx: Option<Receiver<Result<Loaded, String>>>,
    pub(crate) loading: bool,

    pub(crate) thumb_bases: Vec<Option<emulsion_core::image::LinearImage>>,
    pub(crate) thumb_textures: Vec<Option<egui::TextureHandle>>,
    pub(crate) thumb_sizes: Vec<Option<(usize, usize)>>,
    pub(crate) thumb_params: Vec<Option<PipelineParams>>,
    pub(crate) thumb_rx: Option<Receiver<ThumbMessage>>,
    pub(crate) thumb_cancel: Option<Arc<AtomicBool>>,
    pub(crate) thumb_generation: u64,
    pub(crate) thumbs_pending: Vec<usize>,
    pub(crate) thumb_queued: Vec<bool>,

    pub(crate) params: PipelineParams,
    pub(crate) settings: HashMap<String, PipelineParams>,
    // Reused develop buffers so a slider tick allocates nothing.
    pub(crate) preview_scratch: emulsion_core::image::LinearImage,
    pub(crate) preview_out: emulsion_core::image::RgbaImage,
    pub(crate) texture: Option<egui::TextureHandle>,
    pub(crate) texture_size: Option<Vec2>,
    pub(crate) zoom: f32,
    pub(crate) fit_zoom: f32,
    pub(crate) last_stage: Vec2,
    pub(crate) pan: Vec2,
    // Cached stage backdrop, rebuilt only when the canvas rect changes.
    pub(crate) stage_bg: Option<(egui::Rect, Vec<egui::Shape>)>,

    pub(crate) open_tool: Option<Tool>,
    pub(crate) tool_search: String,
    pub(crate) focus_search: bool,
    pub(crate) film_search: String,
    pub(crate) film_focus_search: bool,
    pub(crate) film_textures: HashMap<Option<u16>, egui::TextureHandle>,
    pub(crate) film_preview_base: Option<emulsion_core::image::LinearImage>,
    pub(crate) film_preview_params: Option<PipelineParams>,
    pub(crate) film_preview_source: Option<usize>,
    pub(crate) histogram: Option<Histogram>,

    pub(crate) logo: Option<egui::TextureHandle>,
    pub(crate) wordmark_family: FontFamily,
    pub(crate) status: String,
    pub(crate) toast: Option<(String, Instant)>,
    pub(crate) dirty: bool,
    pub(crate) clipboard: Option<PipelineParams>,
    pub(crate) export_dialog: bool,

    // JPEG export quality (1..=100), default high.
    pub(crate) export_quality: u8,

    // Crop state
    pub(crate) crop_rect: Option<CropRect>, // live crop (mirrors params.crop)
    pub(crate) crop_drag: CropDrag,         // current drag state
    pub(crate) crop_drag_start: Option<Pos2>, // drag start position
    pub(crate) crop_drag_rect: Option<CropRect>, // rect at drag start
    pub(crate) crop_aspect: CropAspect,     // aspect-ratio constraint
    pub(crate) crop_orientation: Orientation, // portrait / landscape
    pub(crate) crop_pan: bool,              // current drag started outside the crop (pan)
}

impl EmulsionApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let wordmark_family = install_fonts(&cc.egui_ctx);
        install_style(&cc.egui_ctx);
        let logo = load_logo(&cc.egui_ctx);
        Self {
            files: Vec::new(),
            tree: Vec::new(),
            expanded: HashSet::new(),
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
            thumbs_pending: Vec::new(),
            thumb_queued: Vec::new(),
            params: PipelineParams::default(),
            settings: HashMap::new(),
            preview_scratch: emulsion_core::image::LinearImage::new(0, 0),
            preview_out: emulsion_core::image::RgbaImage::new(0, 0),
            texture: None,
            texture_size: None,
            zoom: 0.0,
            fit_zoom: 0.0,
            last_stage: Vec2::ZERO,
            pan: Vec2::ZERO,
            stage_bg: None,
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
            status: String::from(OPEN_FOLDER_HINT),
            toast: None,
            dirty: true,
            clipboard: None,
            export_dialog: false,
            export_quality: 92,
            crop_rect: None,
            crop_drag: CropDrag::None,
            crop_drag_start: None,
            crop_drag_rect: None,
            crop_aspect: CropAspect::Free,
            crop_orientation: Orientation::Landscape,
            crop_pan: false,
        }
    }

    pub(crate) fn reset_all(&mut self) {
        self.params = PipelineParams::default();
        self.dirty = true;
        self.show_toast("Edits reset");
    }

    /// Copy the settings of the file at `index` (its live params if it is the
    /// current file, else its stored params) — the row that was right-clicked.
    pub(crate) fn copy_settings_from(&mut self, index: usize) {
        let Some(path) = self.files.get(index) else {
            return;
        };
        if self.cursor == Some(index) {
            self.clipboard = Some(self.params);
        } else {
            self.clipboard = Some(self.settings.get(path).copied().unwrap_or_default());
        }
        self.show_toast("Settings copied");
    }

    /// Paste the clipboard into the file at `index`. The current file is edited
    /// live; any other file just gets its stored params (+ a re-rendered row).
    pub(crate) fn paste_settings_to(&mut self, index: usize) {
        let Some(p) = self.clipboard else { return };
        if self.cursor == Some(index) {
            self.params = p;
            self.crop_rect = p.crop;
            self.crop_drag = CropDrag::None;
            self.crop_drag_start = None;
            self.crop_drag_rect = None;
            self.dirty = true;
        } else if let Some(path) = self.files.get(index) {
            self.settings.insert(path.clone(), p);
        }
        self.enqueue_thumbnail(index);
        self.show_toast("Settings pasted");
    }

    /// Copy: command+C / Cut: command+X / Paste: command+V, from raw key events
    /// (NativeOptions::clipboard_shortcuts is off so egui-winit hands the keys
    /// through instead of turning them into clipboard events).
    fn command_keys(events: &[egui::Event]) -> (bool, bool, bool) {
        let mut keys = (false, false, false);
        for event in events {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
                && modifiers.command
            {
                match key {
                    egui::Key::C => keys.0 = true,
                    egui::Key::X => keys.1 = true,
                    egui::Key::V => keys.2 = true,
                    _ => {}
                }
            }
        }
        keys
    }

    /// Global Ctrl/Cmd+C and Ctrl/Cmd+V act on the selected file (the row
    /// context menu is the per-row way; the hotkeys have no row to target).
    /// Must run before the panels: a focused text field needs its native
    /// clipboard events re-forged from the raw keys so the TextEdit still
    /// works, and that only matters if the events land before it renders.
    fn handle_shortcuts(&mut self, ui: &mut egui::Ui) {
        if self.export_dialog {
            return;
        }
        let (copy, cut, paste) = ui.input(|i| Self::command_keys(&i.events));
        if !(copy || cut || paste) {
            return;
        }
        if ui.ctx().text_edit_focused() {
            let mut events = Vec::new();
            if copy {
                events.push(egui::Event::Copy);
            }
            if cut {
                events.push(egui::Event::Cut);
            }
            // Paste is dropped here: we would need the OS clipboard text and
            // clipboard_shortcuts=false means egui-winit no longer fetches it.
            if !events.is_empty() {
                ui.input_mut(|i| i.events.append(&mut events));
            }
            return;
        }
        let Some(index) = self.cursor else { return };
        if copy {
            self.copy_settings_from(index);
        }
        if paste {
            self.paste_settings_to(index);
        }
    }
}

impl eframe::App for EmulsionApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui);
        self.poll_load();
        self.poll_thumbnails();
        if self.thumb_rx.is_some() || self.load_rx.is_some() {
            ui.ctx().request_repaint();
        }
        if self.dirty {
            self.sync_texture(ui);
        }
        if !self.thumbs_pending.is_empty() {
            self.sync_thumbnails(ui.ctx());
        }

        ui.style_mut().always_scroll_the_only_direction = true;

        egui::Panel::top("top_panel")
            .exact_size(crate::theme::TOPBAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::SETTINGS_BG)
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::left("file_panel")
            .exact_size(crate::theme::SIDEBAR_WIDTH)
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(13, 16)),
            )
            .show(ui, |ui| self.sidebar(ui));

        let settings_height = match self.open_tool {
            Some(Tool::Simulations) => crate::theme::SETTINGS_SIMULATIONS_HEIGHT,
            Some(_) => crate::theme::SETTINGS_OPEN_HEIGHT,
            None => crate::theme::SETTINGS_HEIGHT,
        };
        egui::Panel::top("settings_panel")
            .exact_size(settings_height)
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::SETTINGS_BG)
                    .inner_margin(egui::Margin::symmetric(16, 11)),
            )
            .show(ui, |ui| self.settings_panel(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::STAGE_OUT)
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ui, |ui| self.stage(ui));

        self.paint_toast(ui);
        self.export_modal(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::EmulsionApp;
    use eframe::egui;

    fn key(key: egui::Key, command: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: if command {
                egui::Modifiers::COMMAND
            } else {
                egui::Modifiers::NONE
            },
        }
    }

    #[test]
    fn command_keys_detect_copy_cut_paste() {
        assert_eq!(
            EmulsionApp::command_keys(&[key(egui::Key::C, true)]),
            (true, false, false)
        );
        assert_eq!(
            EmulsionApp::command_keys(&[key(egui::Key::X, true)]),
            (false, true, false)
        );
        assert_eq!(
            EmulsionApp::command_keys(&[key(egui::Key::V, true)]),
            (false, false, true)
        );
        assert_eq!(
            EmulsionApp::command_keys(&[key(egui::Key::C, false)]),
            (false, false, false)
        );
        assert_eq!(
            EmulsionApp::command_keys(&[key(egui::Key::C, true), key(egui::Key::V, true)]),
            (true, false, true)
        );
        // Presses only: a release must not count.
        let release = egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        assert_eq!(EmulsionApp::command_keys(&[release]), (false, false, false));
    }

    #[test]
    fn command_keys_ignore_clipboard_events() {
        // With egui-winit's clipboard_shortcuts enabled these would be all we
        // ever see — and they must NOT satisfy the key hotkeys (that reliance
        // is why NativeOptions.clipboard_shortcuts is set to false).
        assert_eq!(
            EmulsionApp::command_keys(&[egui::Event::Copy]),
            (false, false, false)
        );
        assert_eq!(
            EmulsionApp::command_keys(&[egui::Event::Paste("x".into())]),
            (false, false, false)
        );
    }
}
