use rayon::prelude::*;

use crate::{
    demosaic::demosaic_bilinear,
    error::{CoreError, Result},
    image::LinearImage,
};

use rawler::imgop::matrix::{multiply, normalize, pseudo_inverse};
use rawler::imgop::xyz::{Illuminant, SRGB_TO_XYZ_D65};

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

/// A quickly-decoded small image plus the source's true dimensions. `image`'s
/// own `width/height` are the downscaled ones; `size` is always the real
/// sensor/frame resolution (for metadata like the footer and crop defaults).
pub struct ThumbnailImage {
    pub image: LinearImage,
    pub size: (usize, usize),
}

/// Fast `decode` for small previews: RAW is sub-sampled on the Bayer grid
/// *before* demosaicing (the full-frame demosaic + f32 buffer is the slow
/// part of `decode`), regular images use the `image` crate's `thumbnail`
/// resize. `max_edge` caps the longest edge of the returned image.
pub fn decode_thumbnail(path: &str, max_edge: usize) -> Result<ThumbnailImage> {
    if is_raw_file(path) {
        decode_raw_thumbnail(path, max_edge)
    } else {
        decode_regular_thumbnail(path, max_edge)
    }
}

/// Sampling stride for a `max_edge`-capped thumbnail. The stride keeps the CFA
/// phase intact on the sampled grid: a point `(y*s, x*s)` must carry the same
/// colour as `(y, x)`. For the 2x2/6x6/2x8/12x12 patterns lcm(w,h) == max(w,h),
/// so `stride ≡ 1 (mod period)` covers every supported CFA. (An even stride
/// silently leaves a single Bayer colour in the sample, killing all chroma.)
pub(crate) fn thumbnail_stride(
    width: usize,
    height: usize,
    cfa: &rawler::CFA,
    max_edge: usize,
) -> usize {
    let period = cfa.width.max(cfa.height).max(1);
    let longest = width.max(height) as f32;
    let mut stride = (longest / max_edge.max(1) as f32).max(1.0).ceil() as usize;
    while stride > 1 && !(stride - 1).is_multiple_of(period) {
        stride += 1;
    }
    stride
}

fn decode_raw_thumbnail(path: &str, max_edge: usize) -> Result<ThumbnailImage> {
    let raw = rawler::decode_file(path).map_err(|e| CoreError::Io(format!("rawler: {}", e)))?;
    let width = raw.width;
    let height = raw.height;
    let cfa = &raw.camera.cfa;
    let pixels = raw.pixels_u16();
    let stride = thumbnail_stride(width, height, cfa, max_edge);
    let sw = width.div_ceil(stride);
    let sh = height.div_ceil(stride);
    let mut mini = Vec::with_capacity(sw * sh);
    for y in 0..sh {
        let sy = (y * stride).min(height - 1);
        let row_off = sy * width;
        for x in 0..sw {
            let sx = (x * stride).min(width - 1);
            mini.push(pixels[row_off + sx]);
        }
    }
    let mut img = demosaic_bilinear(sw, sh, &mini, cfa);
    if let Some(matrix) = raw.camera.color_matrix.get(&Illuminant::D65)
        && let Some(cam2rgb) = camera_matrix_to_srgb(matrix)
    {
        apply_camera_matrix(&mut img, &cam2rgb);
    }
    Ok(ThumbnailImage {
        image: img,
        size: (width, height),
    })
}

fn decode_regular_thumbnail(path: &str, max_edge: usize) -> Result<ThumbnailImage> {
    let dimage = image::open(path).map_err(|e| CoreError::Io(format!("read: {}", e)))?;
    let size = (dimage.width() as usize, dimage.height() as usize);
    let small = dimage.thumbnail(max_edge.max(1) as u32, max_edge.max(1) as u32);
    let rgba = small.into_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
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
    Ok(ThumbnailImage { image: out, size })
}

fn decode_raw(path: &str) -> Result<LinearImage> {
    let raw = rawler::decode_file(path).map_err(|e| CoreError::Io(format!("rawler: {}", e)))?;
    let width = raw.width;
    let height = raw.height;
    let cfa = &raw.camera.cfa;
    let pixels = raw.pixels_u16();
    let mut img = demosaic_bilinear(width, height, pixels, cfa);

    // Camera colour matrix: sensor space -> linear sRGB, D65 illuminant.
    // Applied after demosaic's robust auto-scale, before WB gains in develop().
    // Missing matrix -> skip, image stays as demosaiced (current behaviour).
    if let Some(matrix) = raw.camera.color_matrix.get(&Illuminant::D65)
        && let Some(cam2rgb) = camera_matrix_to_srgb(matrix)
    {
        apply_camera_matrix(&mut img, &cam2rgb);
    }

    Ok(img)
}

/// Convert rawler's `FlatColorMatrix` (XYZ -> CAM, 9 floats row-major) to a
/// CAM -> linear sRGB 3x3 matrix.
///
/// `color_matrix` is XYZ->CAM. We compute `rgb2cam = color_matrix *
/// SRGB_TO_XYZ_D65` (sRGB -> CAM), then invert to get CAM -> sRGB.
pub(crate) fn camera_matrix_to_srgb(matrix: &[f32]) -> Option<[[f32; 3]; 3]> {
    if matrix.len() != 9 {
        return None;
    }
    let xyz2cam = [
        [matrix[0], matrix[1], matrix[2]],
        [matrix[3], matrix[4], matrix[5]],
        [matrix[6], matrix[7], matrix[8]],
    ];
    // rgb2cam = xyz2cam * SRGB_TO_XYZ_D65  => sRGB -> CAM
    let rgb2cam = normalize(multiply(&xyz2cam, &SRGB_TO_XYZ_D65));
    // cam2rgb = inverse(rgb2cam)            => CAM -> sRGB
    Some(pseudo_inverse(rgb2cam))
}

/// Apply a 3x3 CAM -> linear sRGB matrix to every pixel of `img`, in place.
pub(crate) fn apply_camera_matrix(img: &mut LinearImage, m: &[[f32; 3]; 3]) {
    img.data.par_chunks_exact_mut(3).for_each(|pix| {
        let r = pix[0];
        let g = pix[1];
        let b = pix[2];
        pix[0] = m[0][0] * r + m[0][1] * g + m[0][2] * b;
        pix[1] = m[1][0] * r + m[1][1] * g + m[1][2] * b;
        pix[2] = m[2][0] * r + m[2][1] * g + m[2][2] * b;
    });
}

fn decode_regular(path: &str) -> Result<LinearImage> {
    let dimage = image::open(path).map_err(|e| CoreError::Io(format!("read: {}", e)))?;
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
