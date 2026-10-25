//! Pure crop geometry: fractional <-> screen mapping and drag/resize math.
//! Kept free of egui widgets so the drag/resize rules can be unit-tested.

use eframe::egui;
use egui::{pos2, Pos2, Rect, Vec2};
use emulsion_core::params::CropRect;

use crate::model::CropDrag;

/// Fractional crop -> screen rectangle inside `frame`.
pub(crate) fn crop_to_screen(cr: CropRect, frame: Rect) -> Rect {
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

/// Fractional crop center -> screen point inside `frame`.
pub(crate) fn crop_center_in_stage(cr: CropRect, frame: Rect) -> Pos2 {
    pos2(
        frame.left() + (cr.x + cr.width * 0.5) * frame.width(),
        frame.top() + (cr.y + cr.height * 0.5) * frame.height(),
    )
}

/// Two arbitrary drag points -> a normalized fractional rect, clamped to
/// `[0,1]` and with non-negative size.
pub(crate) fn crop_from_drag(a: Pos2, b: Pos2, frame: Rect) -> CropRect {
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

/// `None` for a stray click (tiny rect) or a rect covering the whole frame —
/// both mean "no crop".
pub(crate) fn normalize_crop(cr: CropRect) -> Option<CropRect> {
    if cr.width < 0.02 || cr.height < 0.02 {
        return None;
    }
    if cr.x <= 0.001 && cr.y <= 0.001 && cr.width >= 0.999 && cr.height >= 0.999 {
        return None;
    }
    Some(cr)
}

/// Move/resize a handle freely (unconstrained aspect), `nd` in fractional units.
pub(crate) fn free_resize(handle: CropDrag, start: CropRect, nd: Vec2) -> CropRect {
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
/// corner (or edge) stays put. `mouse` is the normalized pointer position.
pub(crate) fn ratio_resize(
    handle: CropDrag,
    start: CropRect,
    mouse: (f32, f32),
    r: f32,
) -> CropRect {
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
/// and staying inside the unit square. Both inputs are normalized `(0..1)`.
pub(crate) fn ratio_draw(anchor: (f32, f32), mouse: (f32, f32), r: f32) -> CropRect {
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    fn cr(x: f32, y: f32, w: f32, h: f32) -> CropRect {
        CropRect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn normalize_drops_tiny_and_full_frame() {
        assert!(normalize_crop(cr(0.5, 0.5, 0.005, 0.5)).is_none());
        assert!(normalize_crop(cr(0.0, 0.0, 1.0, 1.0)).is_none());
        let kept = normalize_crop(cr(0.1, 0.2, 0.5, 0.4)).unwrap();
        assert_eq!(kept, cr(0.1, 0.2, 0.5, 0.4));
    }

    #[test]
    fn crop_from_drag_normalizes_and_clamps() {
        let frame = Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(300.0, 300.0));
        // Reverse drag: down-right to up-left still yields a valid rect.
        let out = crop_from_drag(egui::pos2(280.0, 260.0), egui::pos2(120.0, 140.0), frame);
        assert!((out.x - 0.1).abs() < 1e-5 && (out.y - 0.2).abs() < 1e-5);
        assert!((out.width - 0.8).abs() < 1e-5 && (out.height - 0.6).abs() < 1e-5);
    }

    #[test]
    fn free_resize_br_grows_and_clamps() {
        let start = cr(0.1, 0.1, 0.5, 0.5);
        let out = free_resize(CropDrag::BR, start, egui::vec2(1.0, 1.0));
        assert!((out.x - 0.1).abs() < 1e-5 && (out.width - 0.9).abs() < 1e-5);
        let out = free_resize(CropDrag::BR, start, egui::vec2(-0.3, -0.3));
        assert!((out.width - 0.2).abs() < 1e-5 && (out.height - 0.2).abs() < 1e-5);
    }

    #[test]
    fn free_resize_move_changes_position_not_size() {
        let start = cr(0.2, 0.2, 0.4, 0.3);
        let out = free_resize(CropDrag::Rect, start, egui::vec2(0.1, 0.2));
        assert!((out.x - 0.3).abs() < 1e-5 && (out.y - 0.4).abs() < 1e-5);
        assert!((out.width - 0.4).abs() < 1e-5 && (out.height - 0.3).abs() < 1e-5);
    }

    #[test]
    fn ratio_resize_keeps_aspect() {
        let start = cr(0.4, 0.4, 0.2, 0.2);
        for handle in [CropDrag::BR, CropDrag::TL, CropDrag::TR, CropDrag::BL] {
            let out = ratio_resize(handle, start, (0.7, 0.6), 1.0);
            assert!(
                (out.width * 1.0 - out.height).abs() < 1e-4,
                "aspect broke for {handle:?}"
            );
            assert!(out.x >= -1e-6 && out.y >= -1e-6);
        }
        // r = 2 (2:1 wide) held by both sides.
        let out = ratio_resize(CropDrag::BR, start, (0.6, 0.8), 2.0);
        assert!((out.width / out.height - 2.0).abs() < 1e-4);
    }

    #[test]
    fn ratio_draw_stays_in_frame_and_ratio() {
        let out = ratio_draw((0.1, 0.1), (0.9, 0.5), 1.0);
        assert!((out.width - out.height).abs() < 1e-4);
        assert!(out.x >= 0.0 && out.y >= 0.0 && out.x + out.width <= 1.0 + 1e-4);
        // Dragging up-left from the corner clamps to the frame edge.
        let out = ratio_draw((0.1, 0.1), (0.0, 0.0), 1.0);
        assert!(out.x >= 0.0 && out.y >= 0.0);
        assert!((out.width - out.height).abs() < 1e-4);
    }

    #[test]
    fn screen_mapping_roundtrips() {
        let frame = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 100.0));
        let screen = crop_to_screen(cr(0.25, 0.0, 0.5, 0.5), frame);
        assert_eq!(
            screen,
            Rect::from_min_max(egui::pos2(50.0, 0.0), egui::pos2(150.0, 50.0))
        );
        let center = crop_center_in_stage(cr(0.25, 0.0, 0.5, 0.5), frame);
        assert_eq!(center, egui::pos2(100.0, 25.0));
    }
}
