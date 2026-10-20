use rayon::prelude::*;

use crate::{
    colorops::{apply_black_point, apply_exposure, apply_wb, invert, wb_gains},
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

/// Convert a linear-light `LinearImage` to an 8-bit `RgbaImage`,
/// applying the sRGB gamma (the final step of the pipeline).
pub fn tonemap_and_quantize(img: &LinearImage, out: &mut RgbaImage) {
    let src = &img.data;
    out.data
        .par_chunks_exact_mut(4)
        .enumerate()
        .for_each(|(i, rgba)| {
            let pix = i * 3;
            let r = srgb_encode(src[pix].clamp(0.0, 1.0));
            let g = srgb_encode(src[pix + 1].clamp(0.0, 1.0));
            let b = srgb_encode(src[pix + 2].clamp(0.0, 1.0));
            rgba[0] = (r * 255.0).round() as u8;
            rgba[1] = (g * 255.0).round() as u8;
            rgba[2] = (b * 255.0).round() as u8;
            rgba[3] = 255;
        });
}

/// Full interactive path: apply the param sliders to a developed base and
/// produce the display/export 8-bit image. Order is WB -> exposure ->
/// black point -> invert -> crop/rotate (cheap ops, re-run live).
pub fn develop(base: &LinearImage, params: &PipelineParams) -> RgbaImage {
    let mut working = base.clone();

    apply_wb(
        &mut working,
        &wb_gains(params.wb_temperature, params.wb_tint),
    );
    apply_exposure(&mut working, params.exposure);
    apply_black_point(&mut working, &params.black_point);
    if params.invert {
        invert(&mut working);
    }

    let mut img = match params.crop {
        Some(crop) => crop_image(&working, crop.x, crop.y, crop.width, crop.height),
        None => working,
    };
    img = rotate_90(&img, params.rotate_90);
    img = rotate_fine(&img, params.fine_angle);

    let mut out = RgbaImage::new(img.width, img.height);
    tonemap_and_quantize(&img, &mut out);
    out
}
