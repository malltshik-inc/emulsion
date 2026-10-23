use rayon::prelude::*;

use crate::{
    colorops::{apply_black_point, apply_exposure, apply_wb, wb_gains},
    film::{self, HaldLut},
    geometry::{crop as crop_image, rotate_90, rotate_fine},
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

/// Contrast and saturation, applied to sRGB-encoded values.
///
/// Contrast is a smooth S-curve blended by `contrast/100`: positive values
/// steepen the midtones (without clipping), negative values flatten them
/// toward mid-grey. Saturation scales the distance from Rec.709 luma by
/// `1 + saturation/100` (0 = greyscale, 2 = double saturation).
fn apply_look(mut rgb: [f32; 3], contrast: f32, saturation: f32) -> [f32; 3] {
    let amount = (contrast / 100.0).clamp(-1.0, 1.0);
    if amount != 0.0 {
        for v in &mut rgb {
            let x = v.clamp(0.0, 1.0);
            let s_curve = x * x * (3.0 - 2.0 * x);
            *v = (x + (s_curve - x) * amount).clamp(0.0, 1.0);
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
    tonemap_and_quantize_look(img, 0.0, 0.0, None, out);
}

fn tonemap_and_quantize_look(
    img: &LinearImage,
    contrast: f32,
    saturation: f32,
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
            let mut rgb = apply_look(encoded, contrast, saturation);
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
pub fn develop(base: &LinearImage, params: &PipelineParams) -> RgbaImage {
    let gains = wb_gains(params.wb_temperature, params.wb_tint);
    let mut working = base.clone();

    // A converted negative is inverted below, so its WB gains are applied to
    // the *positive* instead (otherwise +temperature would cool the result).
    if !params.invert {
        apply_wb(&mut working, &gains);
    }
    apply_black_point(&mut working, &params.black_point);

    let mut img = match params.crop {
        Some(crop) => crop_image(&working, crop.x, crop.y, crop.width, crop.height),
        None => working,
    };
    if params.rotate_90 != 0 {
        img = rotate_90(&img, params.rotate_90);
    }
    if params.fine_angle != 0.0 {
        img = rotate_fine(&img, params.fine_angle);
    }

    let film = params.film.and_then(|id| film::lut(id).ok());
    let mut out = RgbaImage::new(img.width, img.height);
    if params.invert {
        invert_negative(
            &img,
            &gains,
            params.exposure,
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
            film.as_deref(),
            &mut out,
        );
    }
    out
}

/// Negative -> positive output: invert in the sRGB-encoded (perceptual)
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
            let mut rgb = apply_look(encoded, params.contrast, params.saturation);
            if let Some(lut) = film {
                rgb = lut.sample(rgb);
            }
            write_rgba(rgb, rgba);
        });
}
