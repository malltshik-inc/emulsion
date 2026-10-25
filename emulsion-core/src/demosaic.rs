use rayon::prelude::*;

use rawler::cfa::CFA;

use crate::image::LinearImage;

/// Caps the sample set fed to the robust-bounds sort (≈256k points).
/// Large enough for stable percentiles, small enough to sort in a few ms.
const MAX_BOUND_SAMPLES: usize = 262_144;

/// Per-channel robust extrema of a Bayer buffer: the 0.25% and 99.75%
/// percentiles of each channel's own samples.
///
/// Raw min/max lets a single hot or dead pixel stretch a channel's whole
/// range, crushing everything else into a dark sliver (and, after inversion,
/// turning the shadows pure black). Percentiles make the measured auto-scale
/// immune to a handful of outliers while keeping the "stretch the scan to the
/// full visible range" behaviour.
///
/// Sampling walks a 2D grid with a step coprime to the CFA period (2x2 Bayer,
/// 6x6 X-Trans), so every colour is represented. A flattened index stride can
/// be a multiple of the pattern period and silently sample one colour only,
/// which leaves that channel's bounds at 0..1 and floods the result with one
/// primary (the ARW "yellow positive / blue negative" bug).
pub fn robust_bounds(
    width: usize,
    height: usize,
    bayer: &[u16],
    cfa: &CFA,
) -> ([f32; 3], [f32; 3]) {
    let mut step = (((width * height) as f32 / MAX_BOUND_SAMPLES as f32).sqrt() as usize).max(1);
    while step.is_multiple_of(2) || step.is_multiple_of(3) {
        step += 1;
    }
    let mut samples = [vec![0_u16; 0], vec![0_u16; 0], vec![0_u16; 0]];
    for y in (0..height).step_by(step) {
        for x in (0..width).step_by(step) {
            let c = cfa.color_at(y, x);
            samples[c].push(bayer[y * width + x]);
        }
    }

    let mut lo = [0.0_f32; 3];
    let mut hi = [0.0_f32; 3];
    for c in 0..3 {
        let s = &mut samples[c];
        s.sort();
        let n = s.len();
        if n == 0 {
            lo[c] = 0.0;
            hi[c] = 1.0;
            continue;
        }
        let k = (n * 25 / 10000).clamp(0, n - 1);
        let l = s[k] as f32;
        let h = s[n - 1 - k] as f32;
        // Guard against flat channels.
        lo[c] = l;
        hi[c] = if h <= l { l + 1.0 } else { h };
    }
    (lo, hi)
}

/// Bilinear demosaic of a single-plane Bayer buffer into an RGB `LinearImage`.
///
/// Each output channel is normalized by the *robust* per-channel 0.25%/99.75%
/// percentile range of that channel's own samples (see [`robust_bounds`]).
/// This is what makes a dark scan viewable: a nominal whitelevel-based scale
/// is wrong when the scan uses only part of the sensor's range, and outlying
/// hot/dead pixels must not be allowed to crush the rest. Output values are
/// linear-light 0.0..1.0.
pub fn demosaic_bilinear(width: usize, height: usize, bayer: &[u16], cfa: &CFA) -> LinearImage {
    let (lo, hi) = robust_bounds(width, height, bayer, cfa);
    let span = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];

    // Precompute the 3x3 neighbour offsets that carry each CFA colour, per
    // pattern phase. The colour of a neighbour depends only on the phase, so
    // the hot loop below never calls `color_at` again.
    let pw = cfa.width.max(1);
    let ph = cfa.height.max(1);
    let phases = ph * pw;
    let mut offsets = vec![[(0_i32, 0_i32); 9]; phases * 3];
    let mut counts = vec![[0_u8; 3]; phases];
    for py in 0..ph {
        for px in 0..pw {
            let base = (py * pw + px) * 3;
            for c in 0..3 {
                let mut n = 0;
                for dy in -1_i32..=1 {
                    for dx in -1_i32..=1 {
                        let yy = (py as i32 + dy).rem_euclid(ph as i32) as usize;
                        let xx = (px as i32 + dx).rem_euclid(pw as i32) as usize;
                        if cfa.color_at(yy, xx) == c {
                            offsets[base + c][n] = (dx, dy);
                            n += 1;
                        }
                    }
                }
                counts[py * pw + px][c] = n as u8;
            }
        }
    }

    let mut out = LinearImage::new(width, height);
    // Each item is a mutable row slice of `width * 3` f32 (RGB interleaved).
    out.data
        .par_chunks_exact_mut(width * 3)
        .enumerate()
        .for_each(|(y, row)| {
            let py = y % ph;
            for x in 0..width {
                let px = x % pw;
                let base = (py * pw + px) * 3;
                let mut rgb = [0.0_f32; 3];
                for c in 0..3 {
                    let n = counts[py * pw + px][c] as usize;
                    let mut sum = 0.0_f32;
                    let mut cnt = 0.0_f32;
                    for &(dx, dy) in &offsets[base + c][..n] {
                        let yy = y as i64 + dy as i64;
                        let xx = x as i64 + dx as i64;
                        if yy < 0 || yy >= height as i64 {
                            continue;
                        }
                        if xx < 0 || xx >= width as i64 {
                            continue;
                        }
                        sum += bayer[yy as usize * width + xx as usize] as f32;
                        cnt += 1.0;
                    }
                    rgb[c] = if cnt > 0.0 {
                        let v = (sum / cnt - lo[c]) / span[c];
                        v.clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                }
                row[x * 3] = rgb[0];
                row[x * 3 + 1] = rgb[1];
                row[x * 3 + 2] = rgb[2];
            }
        });

    out
}
