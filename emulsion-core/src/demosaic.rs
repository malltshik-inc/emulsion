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
/// full visible range" behaviour. The full buffer is stride-sampled so the
/// sort stays bounded on 64 MP sources.
pub fn robust_bounds(
    width: usize,
    height: usize,
    bayer: &[u16],
    cfa: &CFA,
) -> ([f32; 3], [f32; 3]) {
    let stride = (width * height / MAX_BOUND_SAMPLES).max(1);
    let mut samples = [vec![0_u16; 0], vec![0_u16; 0], vec![0_u16; 0]];
    for y in 0..height {
        for x in 0..width {
            if (y * width + x) % stride != 0 {
                continue;
            }
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

    let mut out = LinearImage::new(width, height);
    // Each item is a mutable row slice of `width * 3` f32 (RGB interleaved).
    out.data
        .par_chunks_exact_mut(width * 3)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..width {
                let mut rgb = [0.0_f32; 3];
                for c in 0..3 {
                    let mut sum = 0.0_f32;
                    let mut cnt = 0.0_f32;
                    for dy in -1..=1 {
                        let yy = y as i64 + dy;
                        if yy < 0 || yy >= height as i64 {
                            continue;
                        }
                        for dx in -1..=1 {
                            let xx = x as i64 + dx;
                            if xx < 0 || xx >= width as i64 {
                                continue;
                            }
                            if (cfa.color_at(yy as usize, xx as usize) as usize) == c {
                                sum += bayer[yy as usize * width + xx as usize] as f32;
                                cnt += 1.0;
                            }
                        }
                    }
                    rgb[c] = if cnt > 0.0 {
                        let v = (sum / cnt - lo[c]) / span[c];
                        if v < 0.0 {
                            0.0
                        } else if v > 1.0 {
                            1.0
                        } else {
                            v
                        }
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
