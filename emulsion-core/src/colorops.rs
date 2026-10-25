use rayon::prelude::*;

use crate::image::LinearImage;

/// Compute per-channel white-balance gains from the temperature/tint sliders.
/// Range -100..100 each; 0 = neutral (gain 1.0). Positive temperature is
/// warmer (more red, less blue); positive tint is magenta.
pub fn wb_gains(temperature: f32, tint: f32) -> [f32; 3] {
    let t = (temperature / 100.0).clamp(-1.0, 1.0) * 0.60;
    let m = (tint / 100.0).clamp(-1.0, 1.0) * 0.15;
    [1.0 + t + m, 1.0 - m, 1.0 - t + m]
}

/// Apply per-channel gains to a linear image, in place.
pub fn apply_wb(img: &mut LinearImage, gains: &[f32; 3]) {
    img.data.par_chunks_exact_mut(3).for_each(|pix| {
        pix[0] *= gains[0];
        pix[1] *= gains[1];
        pix[2] *= gains[2];
    });
}

/// Apply exposure in stops, in place. Linear-light scale 2^ev.
pub fn apply_exposure(img: &mut LinearImage, ev: f32) {
    let gain = 2.0_f32.powf(ev);
    img.data.par_chunks_exact_mut(3).for_each(|pix| {
        pix[0] *= gain;
        pix[1] *= gain;
        pix[2] *= gain;
    });
}

/// Apply per-channel black point (0.0..1.0 of the normalized range), in place.
/// Subtracts and re-expands so whites keep their value.
pub fn apply_black_point(img: &mut LinearImage, bp: &[f32; 3]) {
    img.data.par_chunks_exact_mut(3).for_each(|pix| {
        for c in 0..3 {
            let b = bp[c].clamp(0.0, 0.98);
            if b > 0.0 {
                pix[c] = (pix[c] - b) / (1.0 - b);
            }
        }
    });
}
