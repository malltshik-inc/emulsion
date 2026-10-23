use image::ImageReader;
use rayon::prelude::*;

use crate::{
    demosaic::demosaic_bilinear,
    error::{CoreError, Result},
    image::LinearImage,
};

const RAW_EXTENSIONS: [&str; 16] = [
    ".orf", ".nef", ".cr2", ".cr3", ".arw", ".rw2", ".raf", ".pef", ".srw", ".dng", ".erf", ".kdc",
    ".mrw", ".mos", ".dcr", ".iiq",
];

const REGULAR_EXTENSIONS: [&str; 5] = [".jpg", ".jpeg", ".png", ".tif", ".tiff"];

fn is_raw_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    RAW_EXTENSIONS.iter().any(|e| lower.ends_with(*e))
}

/// True when `decode` knows how to open `path` (RAW or JPEG/PNG/TIFF).
pub fn is_supported(path: &str) -> bool {
    let lower = path.to_lowercase();
    is_raw_file(path) || REGULAR_EXTENSIONS.iter().any(|e| lower.ends_with(*e))
}

// Inverse sRGB (encoded [0,1] -> linear-light).
fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Decode any supported source into linear-light [0,1] `LinearImage`.
/// RAW files go through rawler + our demosaic; everything else is decoded
/// to 8-bit and de-gamma'd so the rest of the pipeline is uniform.
pub fn decode(path: &str) -> Result<LinearImage> {
    if is_raw_file(path) {
        decode_raw(path)
    } else {
        decode_regular(path)
    }
}

fn decode_raw(path: &str) -> Result<LinearImage> {
    let raw = rawler::decode_file(path).map_err(|e| CoreError::Io(format!("rawler: {}", e)))?;
    let width = raw.width;
    let height = raw.height;
    let cfa = &raw.camera.cfa;
    let pixels = raw.pixels_u16();
    Ok(demosaic_bilinear(width, height, pixels, cfa))
}

fn decode_regular(path: &str) -> Result<LinearImage> {
    let dimage = ImageReader::open(path)
        .map_err(|e| CoreError::Io(format!("read: {}", e)))?
        .decode()
        .map_err(|e| CoreError::Io(format!("decode: {}", e)))?;
    let rgba = dimage.into_rgba8();
    let dim = rgba.dimensions();
    let width = dim.0 as usize;
    let height = dim.1 as usize;
    let raw_bytes = rgba.as_raw();

    let mut out = LinearImage::new(width, height);
    out.data
        .par_chunks_exact_mut(3)
        .enumerate()
        .for_each(|(i, pix)| {
            let j = i * 4;
            pix[0] = srgb_decode(raw_bytes[j] as f32 / 255.0);
            pix[1] = srgb_decode(raw_bytes[j + 1] as f32 / 255.0);
            pix[2] = srgb_decode(raw_bytes[j + 2] as f32 / 255.0);
        });
    Ok(out)
}
