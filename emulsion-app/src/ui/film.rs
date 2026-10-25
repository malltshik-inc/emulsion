//! Film simulation search, strip and lazily generated previews.

use std::collections::HashMap;

use eframe::egui;
use egui::{Align, Align2, Layout, Margin, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use emulsion_core::film;
use emulsion_core::params::PipelineParams;
use emulsion_core::tonemap::develop;

use crate::app::EmulsionApp;
use crate::theme::{
    ACCENT, DIM, FILM_CARD_GAP, FILM_CARD_HEIGHT, FILM_CARD_WIDTH, FILM_PRELOAD_MARGIN,
    FILM_THUMB_HEIGHT, LINE, MUTED, TEXT,
};
use crate::util::cover_uv;

impl EmulsionApp {
    pub(crate) fn film_controls(&mut self, ui: &mut egui::Ui, params: &mut PipelineParams) {
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
                .fill(egui::Color32::from_rgba_unmultiplied(255, 252, 245, 180))
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(8.0)
                .inner_margin(Margin::symmetric(10, 0));
            frame.show(ui, |ui| {
                let response = ui.add_sized(
                    vec2(search_width, 28.0),
                    egui::TextEdit::singleline(&mut self.film_search)
                        .hint_text(RichText::new("Search simulations").color(DIM))
                        .font(egui::FontId::proportional(13.0))
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
                if crate::util::reset_icon_button(ui, "Reset film simulation") {
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
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
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

    let thumb = Rect::from_min_size(
        pos2(inner.left() + 4.0, inner.top() + 4.0),
        vec2(inner.width() - 8.0, FILM_THUMB_HEIGHT),
    );
    painter.rect_filled(thumb, 5.0, egui::Color32::from_rgb(233, 225, 209));
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
                egui::FontId::proportional(16.0),
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
        let galley = painter.layout_no_wrap(
            label.to_owned(),
            egui::FontId::monospace(8.0),
            egui::Color32::WHITE,
        );
        let badge = Rect::from_min_size(
            pos2(thumb.left() + 4.0, thumb.top() + 4.0),
            galley.size() + vec2(8.0, 4.0),
        );
        painter.rect_filled(badge, 3.0, egui::Color32::from_black_alpha(130));
        painter.galley(badge.min + vec2(4.0, 2.0), galley, egui::Color32::WHITE);
    }

    let text_rect = Rect::from_min_max(
        pos2(inner.left() + 3.0, thumb.bottom() + 4.0),
        pos2(inner.right() - 3.0, inner.bottom() - 2.0),
    );
    let color = if selected { TEXT } else { MUTED };
    let galley = painter.layout(
        name.to_owned(),
        egui::FontId::proportional(10.5),
        color,
        text_rect.width(),
    );
    painter
        .with_clip_rect(text_rect)
        .galley(text_rect.min, galley, color);

    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use emulsion_core::image::LinearImage;

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
            let mut base = LinearImage::new(2, 2);
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
