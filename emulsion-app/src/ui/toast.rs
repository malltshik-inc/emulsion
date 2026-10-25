//! Transient status toast.

use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Align2, Color32, Margin, RichText, Sense, vec2};

use crate::app::EmulsionApp;
use crate::theme::{SIDEBAR_WIDTH, TOAST_BG};

impl EmulsionApp {
    pub(crate) fn show_toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now() + Duration::from_secs(2)));
    }

    pub(crate) fn paint_toast(&mut self, ui: &mut egui::Ui) {
        let Some((text, until)) = self.toast.clone() else {
            return;
        };
        let now = Instant::now();
        if now >= until {
            self.toast = None;
            return;
        }
        egui::Area::new(egui::Id::unique("emulsion_toast"))
            .anchor(
                Align2::CENTER_BOTTOM,
                vec2(0.0 + (SIDEBAR_WIDTH / 2.0), -24.0),
            )
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
