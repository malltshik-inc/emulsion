//! Palette, layout constants, bundled fonts and egui style.

use std::sync::Arc;

use eframe::egui;
use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, vec2};

pub(crate) const THUMB_EDGE: usize = 96;
pub(crate) const THUMB_DECODE_EDGE: usize = THUMB_EDGE * 4;
pub(crate) const SIDEBAR_WIDTH: f32 = 308.0;
pub(crate) const TOPBAR_HEIGHT: f32 = 62.0;
pub(crate) const SETTINGS_HEIGHT: f32 = 70.0;
pub(crate) const SETTINGS_OPEN_HEIGHT: f32 = 126.0;
pub(crate) const SETTINGS_SIMULATIONS_HEIGHT: f32 = 248.0;
pub(crate) const FILM_CARD_WIDTH: f32 = 96.0;
pub(crate) const FILM_CARD_HEIGHT: f32 = 116.0;
pub(crate) const FILM_CARD_GAP: f32 = 8.0;
pub(crate) const FILM_THUMB_HEIGHT: f32 = 66.0;
pub(crate) const FILM_PRELOAD_CARDS: f32 = 2.0;
pub(crate) const FILM_PRELOAD_MARGIN: f32 = FILM_PRELOAD_CARDS * (FILM_CARD_WIDTH + FILM_CARD_GAP);

pub(crate) const BG: Color32 = Color32::from_rgb(242, 236, 221);
pub(crate) const PANEL: Color32 = Color32::from_rgb(250, 246, 236);
pub(crate) const PANEL_TOP: Color32 = Color32::from_rgb(255, 253, 248);
pub(crate) const SETTINGS_BG: Color32 = Color32::from_rgb(247, 241, 230);
pub(crate) const STAGE_IN: Color32 = Color32::from_rgb(251, 247, 238);
pub(crate) const STAGE_OUT: Color32 = Color32::from_rgb(232, 223, 205);
pub(crate) const LINE: Color32 = Color32::from_rgba_unmultiplied_const(56, 44, 28, 28);
pub(crate) const LINE_STRONG: Color32 = Color32::from_rgba_unmultiplied_const(56, 44, 28, 51);
pub(crate) const TEXT: Color32 = Color32::from_rgb(43, 36, 25);
pub(crate) const MUTED: Color32 = Color32::from_rgb(124, 112, 94);
pub(crate) const DIM: Color32 = Color32::from_rgb(167, 155, 133);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(169, 84, 43);
pub(crate) const ACCENT_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(169, 84, 43, 26);
pub(crate) const ACCENT_LINE: Color32 = Color32::from_rgba_unmultiplied_const(169, 84, 43, 77);
pub(crate) const HOVER: Color32 = Color32::from_rgba_unmultiplied_const(56, 44, 28, 13);
pub(crate) const TOAST_BG: Color32 = Color32::from_rgb(43, 36, 25);

pub(crate) fn install_fonts(ctx: &egui::Context) -> FontFamily {
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

pub(crate) fn install_style(ctx: &egui::Context) {
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

pub(crate) fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(include_bytes!("../assets/logo.png")).ok()?;
    let image = image.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture("emulsion-logo", color, egui::TextureOptions::LINEAR))
}

pub(crate) fn window_icon() -> egui::IconData {
    match image::load_from_memory(include_bytes!("../assets/icons/icon-256.png")) {
        Ok(image) => {
            let image = image.to_rgba8();
            egui::IconData {
                rgba: image.as_raw().clone(),
                width: image.width(),
                height: image.height(),
            }
        }
        Err(_) => egui::IconData::default(),
    }
}
