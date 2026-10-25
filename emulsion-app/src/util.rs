//! Small path, label and geometry helpers.

use eframe::egui;
use egui::{Pos2, Rect, Vec2, pos2};

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
