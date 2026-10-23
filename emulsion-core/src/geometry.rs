use rayon::prelude::*;

use crate::image::LinearImage;

/// Crop to a rectangle given in fractional coordinates of the source.
pub fn crop(img: &LinearImage, x0: f32, y0: f32, w: f32, h: f32) -> LinearImage {
    let sx = (x0 * img.width as f32).round() as usize;
    let sy = (y0 * img.height as f32).round() as usize;
    let sw = (w * img.width as f32).round() as usize;
    let sh = (h * img.height as f32).round() as usize;

    let sx = sx.clamp(0, img.width);
    let sy = sy.clamp(0, img.height);
    let sw = sw.clamp(1, img.width - sx);
    let sh = sh.clamp(1, img.height - sy);

    let mut out = LinearImage::new(sw, sh);
    out.data
        .par_chunks_exact_mut(sw * 3)
        .enumerate()
        .for_each(|(y, row)| {
            let src_start = ((sy + y) * img.width + sx) * 3;
            row[..sw * 3].copy_from_slice(&img.data[src_start..src_start + sw * 3]);
        });
    out
}

/// Rotate by 90-degree quarter turns (0..3), clockwise.
pub fn rotate_90(img: &LinearImage, turns: u8) -> LinearImage {
    match turns % 4 {
        0 => img.clone(),
        1 => rotate_quarter_cw(img),
        2 => rotate_180(img),
        _ => rotate_quarter_ccw(img),
    }
}

fn rotate_quarter_cw(img: &LinearImage) -> LinearImage {
    // Output (x, y) reads source (row = height-1-x, col = y).
    let w = img.height;
    let h = img.width;
    let mut out = LinearImage::new(w, h);
    out.data
        .par_chunks_exact_mut(w * 3)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let (r, g, b) = img.pixel(y, img.height - 1 - x);
                row[x * 3] = r;
                row[x * 3 + 1] = g;
                row[x * 3 + 2] = b;
            }
        });
    out
}

fn rotate_quarter_ccw(img: &LinearImage) -> LinearImage {
    // Output (x, y) reads source (row = x, col = width-1-y).
    let w = img.height;
    let h = img.width;
    let mut out = LinearImage::new(w, h);
    out.data
        .par_chunks_exact_mut(w * 3)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let (r, g, b) = img.pixel(img.width - 1 - y, x);
                row[x * 3] = r;
                row[x * 3 + 1] = g;
                row[x * 3 + 2] = b;
            }
        });
    out
}

fn rotate_180(img: &LinearImage) -> LinearImage {
    let mut out = LinearImage::new(img.width, img.height);
    for i in 0..img.width * img.height {
        let j = (img.height - 1 - i / img.width) * img.width + (img.width - 1 - i % img.width);
        let k = i * 3;
        let m = j * 3;
        out.data[k] = img.data[m];
        out.data[k + 1] = img.data[m + 1];
        out.data[k + 2] = img.data[m + 2];
    }
    out
}

/// Rotate by an arbitrary angle (degrees, clockwise positive) around the
/// center, bilinearly sampled, white fill outside the source.
pub fn rotate_fine(img: &LinearImage, angle_deg: f32) -> LinearImage {
    if angle_deg == 0.0 {
        return img.clone();
    }
    let rad = angle_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();

    let cx = (img.width - 1) as f32 / 2.0;
    let cy = (img.height - 1) as f32 / 2.0;

    let mut out = LinearImage::new(img.width, img.height);

    // Inverse map: for each output pixel, find the source coordinate.
    out.data
        .par_chunks_exact_mut(3)
        .enumerate()
        .for_each(|(i, pix)| {
            let y = i / img.width;
            let x = i % img.width;
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            // inverse rotation by -angle
            let sx = cos * dx + sin * dy + cx;
            let sy = -sin * dx + cos * dy + cy;
            let (r, g, b) = sample_bilinear(img, sx, sy);
            pix[0] = r;
            pix[1] = g;
            pix[2] = b;
        });
    out
}

/// Mirror horizontally: the left column becomes the right column (and vice versa).
pub fn flip_horizontal(img: &LinearImage) -> LinearImage {
    let w = img.width;
    let mut out = LinearImage::new(w, img.height);
    out.data
        .par_chunks_exact_mut(w * 3)
        .enumerate()
        .for_each(|(y, row)| {
            let src_row = &img.data[y * w * 3..(y + 1) * w * 3];
            for x in 0..w {
                let src = (w - 1 - x) * 3;
                row[x * 3] = src_row[src];
                row[x * 3 + 1] = src_row[src + 1];
                row[x * 3 + 2] = src_row[src + 2];
            }
        });
    out
}

fn sample_bilinear(img: &LinearImage, x: f32, y: f32) -> (f32, f32, f32) {
    if x < 0.0 || y < 0.0 || x >= (img.width - 1) as f32 || y >= (img.height - 1) as f32 {
        return (1.0, 1.0, 1.0);
    }
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let (r00, g00, b00) = img.pixel(x0, y0);
    let (r10, g10, b10) = img.pixel(x0 + 1, y0);
    let (r01, g01, b01) = img.pixel(x0, y0 + 1);
    let (r11, g11, b11) = img.pixel(x0 + 1, y0 + 1);
    let r = r00 * (1.0 - fx) * (1.0 - fy)
        + r10 * fx * (1.0 - fy)
        + r01 * (1.0 - fx) * fy
        + r11 * fx * fy;
    let g = g00 * (1.0 - fx) * (1.0 - fy)
        + g10 * fx * (1.0 - fy)
        + g01 * (1.0 - fx) * fy
        + g11 * fx * fy;
    let b = b00 * (1.0 - fx) * (1.0 - fy)
        + b10 * fx * (1.0 - fy)
        + b01 * (1.0 - fx) * fy
        + b11 * fx * fy;
    (r, g, b)
}

/// Box-average downscale to a given maximum edge length, preserving aspect.
/// Used to produce a cheap working copy for live preview/UI.
pub fn downscale(img: &LinearImage, max_edge: usize) -> LinearImage {
    let longest = img.width.max(img.height);
    if longest <= max_edge || max_edge == 0 {
        return img.clone();
    }
    let scale = max_edge as f32 / longest as f32;
    let out_w = (img.width as f32 * scale).round().max(1.0) as usize;
    let out_h = (img.height as f32 * scale).round().max(1.0) as usize;

    // Simple area (box) average: each output pixel averages its source region.
    let mut out = LinearImage::new(out_w, out_h);
    out.data
        .par_chunks_exact_mut(out_w * 3)
        .enumerate()
        .for_each(|(oy, row)| {
            let y0 = oy as f64 * img.height as f64 / out_h as f64;
            let y1 = (oy + 1) as f64 * img.height as f64 / out_h as f64;
            let ys = y0.floor() as usize;
            let ye = (y1.ceil() as usize).clamp(ys + 1, img.height);
            for ox in 0..out_w {
                let x0 = ox as f64 * img.width as f64 / out_w as f64;
                let x1 = (ox + 1) as f64 * img.width as f64 / out_w as f64;
                let xs = x0.floor() as usize;
                let xe = (x1.ceil() as usize).clamp(xs + 1, img.width);
                let mut sum = [0.0_f64; 3];
                for sy in ys..ye {
                    for sx in xs..xe {
                        let i = (sy * img.width + sx) * 3;
                        sum[0] += img.data[i] as f64;
                        sum[1] += img.data[i + 1] as f64;
                        sum[2] += img.data[i + 2] as f64;
                    }
                }
                let n = ((ye - ys) * (xe - xs)) as f64;
                let oi = ox * 3;
                row[oi] = (sum[0] / n) as f32;
                row[oi + 1] = (sum[1] / n) as f32;
                row[oi + 2] = (sum[2] / n) as f32;
            }
        });
    out
}
