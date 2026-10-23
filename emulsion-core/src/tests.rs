#![cfg(test)]

use rayon::prelude::*;

use crate::{
    colorops::{apply_black_point, apply_exposure, apply_wb, invert, wb_gains},
    decode::is_supported,
    demosaic::demosaic_bilinear,
    demosaic::robust_bounds,
    export::save_sidecar,
    geometry::{crop, downscale, rotate_90, rotate_fine},
    image::{LinearImage, RgbaImage},
    params::PipelineParams,
    tonemap::{develop, tonemap_and_quantize},
};

fn make_img(w: usize, h: usize, fill: f32) -> LinearImage {
    let mut img = LinearImage::new(w, h);
    img.data.par_chunks_exact_mut(3).for_each(|pix| {
        pix[0] = fill;
        pix[1] = fill;
        pix[2] = fill;
    });
    img
}

fn close(a: f32, b: f32) -> bool {
    let d = if a >= b { a - b } else { b - a };
    d < 1e-4_f32
}

#[test]
fn export_sidecar_records_source_and_params() {
    let path = std::env::temp_dir().join("emulsion_sidecar_test.json");
    let path = path.to_string_lossy().into_owned();
    let params = PipelineParams {
        invert: true,
        exposure: 1.25,
        contrast: 40.0,
        saturation: 40.0,
        ..PipelineParams::default()
    };
    save_sidecar(&path, r"C:\film\frame.ORF", &params).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["format"], "emulsion-params-v1");
    assert_eq!(value["source"], r"C:\film\frame.ORF");
    assert_eq!(value["params"]["invert"], true);
    assert_eq!(value["params"]["exposure"], 1.25);
    assert_eq!(value["params"]["contrast"], 40.0);
    assert_eq!(value["params"]["saturation"], 40.0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn supported_extension_detection_is_case_insensitive() {
    assert!(is_supported("P5220434.ORF"));
    assert!(is_supported(r"C:\photos\shot.ARW"));
    assert!(is_supported("a.nef"));
    assert!(is_supported("a.JPG"));
    assert!(is_supported("a.png"));
    assert!(!is_supported("notes.txt"));
}

#[test]
fn wb_gains_neutral() {
    let g = wb_gains(0.0, 0.0);
    assert!(g[0] > 0.99 && g[0] < 1.01);
    assert!(g[1] > 0.99 && g[1] < 1.01);
    assert!(g[2] > 0.99 && g[2] < 1.01);
}

#[test]
fn wb_gains_warm_boosts_red() {
    let g = wb_gains(100.0, 0.0);
    assert!(g[0] > g[2]);
}

#[test]
fn apply_wb_scales_planes() {
    let mut img = make_img(2, 2, 0.5);
    let gains = &wb_gains(100.0, 0.0);
    apply_wb(&mut img, gains);
    let (r, g, _) = img.px(0, 0);
    assert!(r > g);
}

#[test]
fn exposure_doubles_at_one_stop() {
    let mut img = make_img(2, 2, 0.25);
    apply_exposure(&mut img, 1.0);
    let (r, g, b) = img.px(0, 0);
    assert!(close(r, 0.5) && close(g, 0.5) && close(b, 0.5));
}

#[test]
fn black_point_shifts_but_preserves_white() {
    let mut img = LinearImage::new(1, 2);
    img.set_px(0, 0, 0.2, 0.2, 0.2);
    img.set_px(0, 1, 1.0, 1.0, 1.0);
    apply_black_point(&mut img, &[0.2, 0.2, 0.2]);
    let (r1, _, _) = img.px(0, 0);
    let (r2, _, _) = img.px(0, 1);
    assert!(close(r1, 0.0));
    assert!(close(r2, 1.0));
}

#[test]
fn invert_flips() {
    let mut img = make_img(1, 1, 0.3);
    invert(&mut img);
    let (r, g, b) = img.px(0, 0);
    assert!(close(r, 0.7) && close(g, 0.7) && close(b, 0.7));
}

#[test]
fn crop_halves() {
    let mut img = LinearImage::new(4, 4);
    for y in 0..4 {
        for x in 0..4 {
            img.set_px(x, y, x as f32, y as f32, 0.0);
        }
    }
    let c = crop(&img, 0.25, 0.25, 0.5, 0.5);
    assert_eq!(c.width, 2);
    assert_eq!(c.height, 2);
}

#[test]
fn rotate_90_cw_corners() {
    // 2x1 image: (0,0)=R (left), (1,0)=G (right).
    let mut img = LinearImage::new(2, 1);
    img.set_px(0, 0, 1.0, 0.0, 0.0);
    img.set_px(1, 0, 0.0, 1.0, 0.0);
    let r = rotate_90(&img, 1); // CW: left edge -> top
    assert_eq!(r.width, 1);
    assert_eq!(r.height, 2);
    // top = original left = R
    let (r0, g0, _) = r.px(0, 0);
    let (r1, g1, _) = r.px(0, 1);
    assert!(close(r0, 1.0) && close(g0, 0.0));
    assert!(close(r1, 0.0) && close(g1, 1.0));
}

#[test]
fn rotate_90_ccw_corners() {
    // 2x1 image: (0,0)=R (left), (1,0)=G (right).
    let mut img = LinearImage::new(2, 1);
    img.set_px(0, 0, 1.0, 0.0, 0.0);
    img.set_px(1, 0, 0.0, 1.0, 0.0);
    let r = rotate_90(&img, 3); // CCW: right edge -> top
    assert_eq!(r.width, 1);
    assert_eq!(r.height, 2);
    let (r0, g0, _) = r.px(0, 0);
    let (r1, g1, _) = r.px(0, 1);
    assert!(close(r0, 0.0) && close(g0, 1.0));
    assert!(close(r1, 1.0) && close(g1, 0.0));
}

#[test]
fn rotate_360_is_identity() {
    let mut img = LinearImage::new(3, 2);
    for y in 0..2 {
        for x in 0..3 {
            img.set_px(x, y, x as f32, y as f32, (x + y) as f32);
        }
    }
    let original = img.clone();
    let r = rotate_90(&img, 4);
    for y in 0..2 {
        for x in 0..3 {
            let (a, b, c) = r.px(x, y);
            let (d, e, f) = original.px(x, y);
            assert!(close(a, d) && close(b, e) && close(c, f));
        }
    }
}

#[test]
fn rotate_fine_zero_is_identity() {
    let mut img = LinearImage::new(3, 3);
    for y in 0..3 {
        for x in 0..3 {
            img.set_px(x, y, x as f32, y as f32, (x + y) as f32);
        }
    }
    let r = rotate_fine(&img, 0.0);
    for y in 0..3 {
        for x in 0..3 {
            let (a, b, c) = r.px(x, y);
            let (d, e, f) = img.px(x, y);
            assert!(close(a, d) && close(b, e) && close(c, f));
        }
    }
}

#[test]
fn rotate_fine_90_keeps_center_whites_corners() {
    let mut img = LinearImage::new(3, 3);
    for y in 0..3 {
        for x in 0..3 {
            img.set_px(x, y, x as f32, y as f32, 0.0);
        }
    }
    // center pixel (1,1) has value x=1
    assert!(close(img.px(1, 1).0, 1.0));
    let r = rotate_fine(&img, 90.0);
    // 90 degree rotation around center is exact: center preserved...
    // but bilinear samples near center, so compare loosely.
    let (rc, _, _) = r.px(1, 1);
    assert!((rc - 1.0).abs() < 0.5);
    // corners sample outside -> white fill (all 1.0)
    let (r0, g0, b0) = r.px(0, 0);
    assert!(r0 > 0.9 && g0 > 0.9 && b0 > 0.9);
}

#[test]
fn tonemap_quantize_produces_rgba() {
    let mut img = LinearImage::new(2, 2);
    img.set_px(0, 0, 0.0, 0.5, 1.0);
    let mut out = RgbaImage::new(2, 2);
    tonemap_and_quantize(&img, &mut out);
    assert_eq!(out.data[0], 0);
    assert!(out.data[1] > 100 && out.data[1] < 200); // gamma of 0.5 -> mid-bright
    assert_eq!(out.data[3], 255);
}

#[test]
fn develop_contrast_expands_or_flattens_midtones() {
    let mut base = make_img(2, 1, 0.2);
    base.data[3..6].fill(0.7);
    let render = |contrast: f32| {
        let params = PipelineParams {
            contrast,
            ..PipelineParams::default()
        };
        develop(&base, &params)
    };
    let neutral = render(0.0);
    let punchy = render(100.0);
    let flat = render(-100.0);

    // Pixel 0 is the dark step, pixel 1 the bright one.
    assert!(punchy.data[0] < neutral.data[0]);
    assert!(punchy.data[4] > neutral.data[4]);
    assert!(flat.data[0] > neutral.data[0]);
    assert!(flat.data[4] < neutral.data[4]);
}

#[test]
fn develop_saturation_scales_chroma() {
    let mut base = make_img(1, 1, 0.0);
    base.data[0] = 0.6;
    base.data[1] = 0.4;
    base.data[2] = 0.4;
    let render = |saturation: f32| {
        let params = PipelineParams {
            saturation,
            ..PipelineParams::default()
        };
        develop(&base, &params)
    };
    let spread = |img: &RgbaImage| img.data[0] as i32 - img.data[2] as i32;

    let neutral = render(0.0);
    let boost = render(100.0);
    let gray = render(-100.0);
    assert!(spread(&boost) > spread(&neutral));
    assert!(spread(&gray).abs() <= 1);
}

#[test]
fn develop_invert_produces_positive_stats() {
    // Raw-like base: dark values (what a negative under a light table looks like).
    // Inversion runs after the sRGB encode, so 0.05 linear -> 0.23 encoded ->
    // 0.77 positive (~196/255). Inverting in linear first would give ~250.
    let base = make_img(4, 4, 0.05);
    let mut params = PipelineParams::default();
    params.invert = true;
    let out = develop(&base, &params);
    let sum: u64 = out
        .data
        .par_iter()
        .map(|b| *b as u64)
        .reduce(|| 0_u64, |a, b| a + b);
    let n = (out.data.len() / 4) as u64;
    let mean = sum / n;
    assert!(mean > 180, "mean {}", mean);
}

#[test]
fn develop_exposure_brightens_converted_negative() {
    // A negative mid-tone: +EV must brighten the displayed positive, -EV darken
    // it (exposure runs *after* inversion).
    let base = make_img(4, 4, 0.4);
    let mean = |exposure: f32| {
        let mut params = PipelineParams::default();
        params.invert = true;
        params.exposure = exposure;
        let out = develop(&base, &params);
        let sum: u64 = out
            .data
            .par_iter()
            .map(|b| *b as u64)
            .reduce(|| 0_u64, |a, b| a + b);
        sum / (out.data.len() / 4) as u64
    };
    let bright = mean(1.0);
    let neutral = mean(0.0);
    let dark = mean(-1.0);
    assert!(
        bright > neutral && neutral > dark,
        "expected bright > neutral > dark, got {} / {} / {}",
        bright,
        neutral,
        dark
    );
}

#[test]
fn develop_wb_temperature_warms_converted_negative() {
    // WB must act on the displayed positive: + warms (R > B), - cools.
    let base = make_img(4, 4, 0.4);
    let rgb_mean = |temperature: f32| {
        let params = PipelineParams {
            invert: true,
            wb_temperature: temperature,
            ..PipelineParams::default()
        };
        let out = develop(&base, &params);
        let mut sums = [0_u64; 3];
        for i in (0..out.data.len()).step_by(4) {
            for (c, sum) in sums.iter_mut().enumerate() {
                *sum += out.data[i + c] as u64;
            }
        }
        let n = (out.data.len() / 4) as u64;
        [sums[0] / n, sums[1] / n, sums[2] / n]
    };
    let warm = rgb_mean(100.0);
    let cool = rgb_mean(-100.0);
    assert!(warm[0] > warm[2], "warm RGB {:?}", warm);
    assert!(cool[2] > cool[0], "cool RGB {:?}", cool);
}

#[test]
fn params_default_serializes_to_json() {
    let p = PipelineParams::default();
    let json = serde_json::to_string_pretty(&p).unwrap();
    let back = serde_json::from_str::<PipelineParams>(&json).unwrap();
    assert_eq!(back.invert, p.invert);
    assert_eq!(back.exposure, p.exposure);
}

#[test]
fn demosaic_robust_bounds_rejects_hot_and_dead_pixels() {
    let w = 64;
    let h = 64;
    let mut bayer = vec![0_u16; w * h];
    for i in 0..w * h {
        bayer[i] = 4000 + (i % 500) as u16;
    }
    // One hot pixel above and one dead pixel below the real data.
    bayer[w * h / 2] = 65535;
    bayer[w * h / 2 + 1] = 0;
    let cfa = rawler::CFA::new("RGGB");
    let (lo, hi) = robust_bounds(w, h, &bayer, &cfa);
    // Raw min/max would report lo=0 and hi=65535; robust bounds stay put.
    for c in 0..3 {
        assert!(lo[c] >= 3990.0 && lo[c] <= 4100.0);
        assert!(hi[c] >= 4400.0 && hi[c] <= 4600.0);
    }
}

#[test]
fn demosaic_robust_bounds_sample_all_cfa_colors() {
    // 1024x1024 / 262144 = a stride of 4 used to align with the 2x2 pattern,
    // sampling only one colour and leaving the other bounds at 0..1.
    let w = 1024;
    let h = 1024;
    let cfa = rawler::CFA::new("RGGB");
    let mut bayer = vec![0_u16; w * h];
    for y in 0..h {
        for x in 0..w {
            bayer[y * w + x] = match cfa.color_at(y, x) {
                0 => 1000,
                1 => 2000,
                _ => 3000,
            };
        }
    }
    let (lo, hi) = robust_bounds(w, h, &bayer, &cfa);
    for (c, expected) in [1000.0, 2000.0, 3000.0].iter().enumerate() {
        assert!(
            (lo[c] - expected).abs() <= 1.0,
            "channel {c} lo {} != {expected}",
            lo[c]
        );
        assert!(
            (hi[c] - expected).abs() <= 1.0,
            "channel {c} hi {} != {expected}",
            hi[c]
        );
    }
}

#[test]
fn demosaic_robust_bounds_do_not_crush_dark_channels() {
    // Data spanning 500..900 per channel, with hot/dead outliers. With raw
    // min/max the hot pixel alone stretches the range 70x, crushing the bulk
    // into a near-black sliver; robust percentiles keep the bulk mid-scaled.
    let w = 128;
    let h = 128;
    let mut bayer = vec![0_u16; w * h];
    for y in 0..h {
        for x in 0..w {
            bayer[y * w + x] = 500 + ((x * 7 + y * 13) % 401) as u16;
        }
    }
    bayer[0] = 65535;
    bayer[1] = 65535;
    bayer[2] = 0;
    bayer[3] = 0;
    let cfa = rawler::CFA::new("RGGB");
    let img = demosaic_bilinear(w, h, &bayer, &cfa);
    let sum: f32 = img
        .data
        .par_iter()
        .map(|v| *v)
        .reduce(|| 0.0_f32, |a, b| a + b);
    let mean = sum / img.data.len() as f32;
    assert!(
        mean > 0.2 && mean < 0.9,
        "mean {} is crushed or blown",
        mean
    );
    let (r, g, b) = img.px(64, 64);
    assert!(r > 0.1 && g > 0.1 && b > 0.1);
}

#[test]
fn demosaic_bilinear_basic_shape() {
    let w = 4;
    let h = 4;
    let bayer = vec![1024_u16; w * h];
    let cfa = rawler::CFA::new("RGGB");
    let img = demosaic_bilinear(w, h, &bayer, &cfa);
    assert_eq!(img.width, w);
    assert_eq!(img.height, h);
}

#[test]
fn downscale_halves_longest_edge() {
    let mut img = LinearImage::new(200, 100);
    for y in 0..100 {
        for x in 0..200 {
            img.set_px(x, y, 0.1, 0.2, 0.3);
        }
    }
    let d = downscale(&img, 100);
    // longest edge 200 -> 100
    assert_eq!(d.width, 100);
    assert_eq!(d.height, 50);
    // average preserved (roughly)
    let (r, g, b) = d.px(50, 25);
    assert!(close(r, 0.1) && close(g, 0.2) && close(b, 0.3));
}
