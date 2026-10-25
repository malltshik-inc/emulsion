use rayon::prelude::*;

use crate::{
    colorops::{apply_black_point, apply_exposure, apply_wb, wb_gains},
    film::{self, HaldLut},
    geometry::{crop as crop_image, flip_horizontal, rotate_90, rotate_fine},
    image::{LinearImage, RgbaImage},
    params::PipelineParams,
};

// Standard sRGB transfer function (linear-light in [0,1] -> encoded [0,1]).
const SRGB_CROSSOVER: f32 = 0.00304;
const LINEAR_GAIN: f32 = 12.92;
const SRGB_GAIN: f32 = 1.055;
const SRGB_OFFSET: f32 = 0.055;
const SRGB_GAMMA: f32 = 2.4;

fn srgb_encode(v: f32) -> f32 {
    if v <= SRGB_CROSSOVER {
        v * LINEAR_GAIN
    } else {
        v.powf(1.0 / SRGB_GAMMA) * SRGB_GAIN - SRGB_OFFSET
    }
}

fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v * LINEAR_GAIN.recip()
    } else {
        ((v + SRGB_OFFSET) / SRGB_GAIN).powf(SRGB_GAMMA)
    }
}

/// Contrast, shadows/highlights and saturation, applied to sRGB-encoded values.
///
/// Contrast is a smooth S-curve blended by `contrast/100`: positive values
/// steepen the midtones (without clipping), negative values flatten them
/// toward mid-grey. Shadows/highlights lift/lower the dark/bright ends with a
/// squared weight. Saturation scales the distance from Rec.709 luma by
/// `1 + saturation/100` (0 = greyscale, 2 = double saturation).
fn apply_look(
    mut rgb: [f32; 3],
    contrast: f32,
    saturation: f32,
    shadows: f32,
    highlights: f32,
) -> [f32; 3] {
    let amount = (contrast / 100.0).clamp(-1.0, 1.0);
    if amount != 0.0 {
        for v in &mut rgb {
            let x = v.clamp(0.0, 1.0);
            let s_curve = x * x * (3.0 - 2.0 * x);
            *v = (x + (s_curve - x) * amount).clamp(0.0, 1.0);
        }
    }

    let sh = (shadows / 100.0).clamp(-1.0, 1.0);
    let hi = (highlights / 100.0).clamp(-1.0, 1.0);
    if sh != 0.0 || hi != 0.0 {
        for v in &mut rgb {
            let x = v.clamp(0.0, 1.0);
            let shadow_w = (1.0 - x) * (1.0 - x);
            let highlight_w = x * x;
            *v = (x + sh * 0.5 * shadow_w + hi * 0.5 * highlight_w).clamp(0.0, 1.0);
        }
    }

    let sat = 1.0 + (saturation / 100.0).clamp(-1.0, 1.0);
    if sat != 1.0 {
        let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
        for v in &mut rgb {
            *v = (luma + (*v - luma) * sat).clamp(0.0, 1.0);
        }
    }
    rgb
}

fn write_rgba(rgb: [f32; 3], rgba: &mut [u8]) {
    rgba[0] = (rgb[0] * 255.0).round() as u8;
    rgba[1] = (rgb[1] * 255.0).round() as u8;
    rgba[2] = (rgb[2] * 255.0).round() as u8;
    rgba[3] = 255;
}

/// Convert a linear-light `LinearImage` to an 8-bit `RgbaImage`,
/// applying the sRGB gamma (the final step of the pipeline).
pub fn tonemap_and_quantize(img: &LinearImage, out: &mut RgbaImage) {
    tonemap_and_quantize_look(img, 0.0, 0.0, 0.0, 0.0, None, out);
}

fn tonemap_and_quantize_look(
    img: &LinearImage,
    contrast: f32,
    saturation: f32,
    shadows: f32,
    highlights: f32,
    film: Option<&HaldLut>,
    out: &mut RgbaImage,
) {
    let src = &img.data;
    out.data
        .par_chunks_exact_mut(4)
        .enumerate()
        .for_each(|(i, rgba)| {
            let pix = i * 3;
            let encoded = [
                srgb_encode(src[pix].clamp(0.0, 1.0)),
                srgb_encode(src[pix + 1].clamp(0.0, 1.0)),
                srgb_encode(src[pix + 2].clamp(0.0, 1.0)),
            ];
            let mut rgb = apply_look(encoded, contrast, saturation, shadows, highlights);
            if let Some(lut) = film {
                rgb = lut.sample(rgb);
            }
            write_rgba(rgb, rgba);
        });
}

/// Full interactive path: apply the param sliders to a developed base and
/// produce the display/export 8-bit image. Order is WB -> black point ->
/// crop/rotate -> invert + exposure (cheap ops, re-run live).
///
/// Black point stays *before* inversion (PRODUCT.md §8: it removes the
/// negative's orange mask). Exposure is applied *after* inversion so +EV
/// brightens the displayed positive rather than darkening it.
///
/// When Develop (invert) is on, the `invert_sensitivity` slider simultaneously raises
/// exposure, contrast and saturation on top of the per-slider values. The
/// invert_sensitivity slider only affects the inverted path; positive (non-inverted)
/// images are untouched by it.
pub fn develop(base: &LinearImage, params: &PipelineParams) -> RgbaImage {
    let gains = wb_gains(params.wb_temperature, params.wb_tint);
    let mut working = base.clone();

    // A converted negative is inverted below, so its WB gains are applied to
    // the *positive* instead (otherwise +temperature would cool the result).
    if !params.invert {
        apply_wb(&mut working, &gains);
    }
    apply_black_point(&mut working, &params.black_point);

    let mut img = working;

    if params.rotate_90 != 0 {
        img = rotate_90(&img, params.rotate_90);
    }
    if params.fine_angle != 0.0 {
        img = rotate_fine(&img, params.fine_angle);
    }
    if params.flip_horizontal {
        img = flip_horizontal(&img);
    }
    if let Some(crop) = params.crop {
        img = crop_image(&img, crop.x, crop.y, crop.width, crop.height);
    }

    let film = params.film.and_then(|id| film::lut(id).ok());
    let mut out = RgbaImage::new(img.width, img.height);
    if params.invert {
        let conversion_gains = wb_gains(params.wb_temperature, params.wb_tint);
        let exposure_ev = params.exposure + params.invert_sensitivity * 0.014;
        invert_negative(
            &img,
            &conversion_gains,
            exposure_ev,
            params,
            film.as_deref(),
            &mut out,
        );
    } else {
        apply_exposure(&mut img, params.exposure);
        tonemap_and_quantize_look(
            &img,
            params.contrast,
            params.saturation,
            params.shadows,
            params.highlights,
            film.as_deref(),
            &mut out,
        );
    }
    if params.vignette != 0.0 {
        apply_vignette(&mut out, params.vignette);
    }
    out
}

/// Negative -> positive invert_sensitivity: invert in the sRGB-encoded (perceptual)
/// domain, then apply WB gains and exposure in linear light.
///
/// A negative's transmittance is roughly `10^-density`, so inverting the
/// linear values *before* the gamma encode lifts the midtones and flattens
/// the print (the "pale and too bright" look). Inverting the encoded values
/// matches how a positive is normally read, keeps blacks black, and leaves
/// the gamma encode out of the inversion.
fn invert_negative(
    img: &LinearImage,
    gains: &[f32; 3],
    exposure_ev: f32,
    params: &PipelineParams,
    film: Option<&HaldLut>,
    out: &mut RgbaImage,
) {
    let exposure = 2.0_f32.powf(exposure_ev);
    let contrast = params.contrast + params.invert_sensitivity * 0.4;
    let saturation = params.saturation + params.invert_sensitivity * 0.2;
    let src = &img.data;
    out.data
        .par_chunks_exact_mut(4)
        .enumerate()
        .for_each(|(i, rgba)| {
            let pix = i * 3;
            let mut encoded = [0.0_f32; 3];
            for (c, value) in encoded.iter_mut().enumerate() {
                let neg = srgb_encode(src[pix + c].clamp(0.0, 1.0));
                let pos = 1.0 - neg;
                let lin = (srgb_decode(pos) * gains[c] * exposure).clamp(0.0, 1.0);
                *value = srgb_encode(lin);
            }
            let mut rgb = apply_look(
                encoded,
                contrast,
                saturation,
                params.shadows,
                params.highlights,
            );
            if let Some(lut) = film {
                rgb = lut.sample(rgb);
            }
            write_rgba(rgb, rgba);
        });
}

/// Darken (positive) or brighten (negative) the corners with a radial falloff.
fn apply_vignette(img: &mut RgbaImage, amount: f32) {
    let w = img.width;
    let h = img.height;
    if w < 2 || h < 2 {
        return;
    }
    let a = (amount / 100.0).clamp(-1.0, 1.0);
    let cx = (w - 1) as f32 / 2.0;
    let cy = (h - 1) as f32 / 2.0;
    let max_d = (cx * cx + cy * cy).sqrt().max(f32::EPSILON);
    img.data
        .par_chunks_exact_mut(4)
        .enumerate()
        .for_each(|(i, px)| {
            let x = (i % w) as f32;
            let y = (i / w) as f32;
            let d = ((x - cx) * (x - cx) + (y - cy) * (y - cy)).sqrt() / max_d;
            let fall = (d * d).clamp(0.0, 1.0);
            let f = (1.0 - a * 0.9 * fall).max(0.0);
            px[0] = (px[0] as f32 * f).round() as u8;
            px[1] = (px[1] as f32 * f).round() as u8;
            px[2] = (px[2] as f32 * f).round() as u8;
        });
}
