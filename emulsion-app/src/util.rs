//! Small path, label and geometry helpers.

use eframe::egui;
use egui::{Pos2, Rect, Vec2, pos2, vec2};

pub(crate) fn file_name(path: &str) -> String {
    match std::path::Path::new(path).file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => String::from(path),
    }
}

pub(crate) fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name(path))
}

pub(crate) fn file_extension(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .map(|s| s.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| String::from("IMAGE"))
}

/// Megapixels to one decimal ("24.3").
pub(crate) fn megapixels(w: usize, h: usize) -> String {
    format!("{:.1}", w as f64 * h as f64 / 1_000_000.0)
}

/// A small transparent "reset" icon button; returns true when clicked.
pub(crate) fn reset_icon_button(ui: &mut egui::Ui, hover: &str) -> bool {
    ui.add(
        egui::Button::new(
            egui::RichText::new("\u{21ba}")
                .size(15.0)
                .color(crate::theme::DIM),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .corner_radius(6.0)
        .min_size(vec2(28.0, 28.0)),
    )
    .on_hover_text(hover)
    .clicked()
}

/// The 52x28 outlined toggle used by the rotate/develop rows: transparent and
/// muted when inactive, accent-tinted when active.
pub(crate) fn small_toggle_button(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(12.0)
                .color(crate::theme::MUTED),
        )
        .fill(if active {
            crate::theme::ACCENT_SOFT
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::new(1.0, crate::theme::LINE))
        .corner_radius(6.0)
        .min_size(vec2(52.0, 28.0)),
    )
}

pub(crate) fn aspect_label(w: usize, h: usize) -> String {
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

pub(crate) fn cover_uv(image: Vec2, target: Vec2) -> Rect {
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

#[cfg(test)]
mod tests {
    use super::{aspect_label, cover_uv};
    use eframe::egui;

    #[test]
    fn aspect_label_matches_known_ratios() {
        assert_eq!(aspect_label(3, 2), "3:2");
        assert_eq!(aspect_label(16, 9), "16:9");
        assert_eq!(aspect_label(4, 4), "1:1");
        assert_eq!(aspect_label(4, 5), "4:5");
        assert_eq!(aspect_label(10, 20), "0.50");
    }

    #[test]
    fn aspect_label_falls_back_to_ratio() {
        assert_eq!(aspect_label(1000, 1), "1000.00");
        assert_eq!(aspect_label(0, 0), "—");
    }

    #[test]
    fn cover_uv_insets_wider_image() {
        // image 2:1, target 1:1 -> x inset 0.25.
        let uv = cover_uv(egui::vec2(200.0, 100.0), egui::vec2(100.0, 100.0));
        assert!((uv.min.x - 0.25).abs() < 1e-6 && (uv.max.x - 0.75).abs() < 1e-6);
        assert!(uv.min.y == 0.0 && uv.max.y == 1.0);
    }

    #[test]
    fn cover_uv_insets_taller_target() {
        // image 1:2, target 1:1 -> y inset 0.25.
        let uv = cover_uv(egui::vec2(100.0, 200.0), egui::vec2(100.0, 100.0));
        assert!(uv.min.x == 0.0 && uv.max.x == 1.0);
        assert!((uv.min.y - 0.25).abs() < 1e-6 && (uv.max.y - 0.75).abs() < 1e-6);
    }
}
