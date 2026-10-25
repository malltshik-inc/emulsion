//! RGB histogram calculation and painting.

use eframe::egui;
use egui::{Color32, Rect, Sense, Stroke, pos2, vec2};

#[derive(Clone)]
pub(crate) struct Histogram {
    channels: [[f32; 64]; 3],
}

pub(crate) fn make_histogram(data: &[u8]) -> Option<Histogram> {
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

pub(crate) fn paint_histogram(ui: &mut egui::Ui, histogram: Option<&Histogram>, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let painter = ui.painter();
    for i in 1..3 {
        let y = rect.top() + rect.height() * i as f32 / 3.0;
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0, crate::theme::LINE),
        );
    }
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, crate::theme::LINE),
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
