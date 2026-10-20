use std::env;

use rawler::RawImage;

const USAGE: &str = "usage: orf_to_png <input.orf> <output.png>";

fn main() {
    let args = env::args().collect::<Vec<String>>();
    if args.len() != 3 {
        eprintln!("{}", USAGE);
        std::process::exit(2);
    }
    let input = &args[1];
    let output = &args[2];

    let raw = rawler::decode_file(input).unwrap();
    println!(
        "decoded {} {}: {}x{} bps={} cpp={}",
        raw.clean_make, raw.clean_model, raw.width, raw.height, raw.bps, raw.cpp
    );
    println!("cfa: {}", raw.camera.cfa);

    let white = raw.whitelevel.as_bayer_array()[0];
    let black = raw.blacklevel.as_bayer_array()[0];
    println!("blacklevel={} whitelevel={}", black, white);

    let pixels = raw.pixels_u16();
    println!(
        "pixel count: {}, expected {}",
        pixels.len(),
        raw.width * raw.height * raw.cpp
    );
    let mut lo = u16::MAX;
    let mut hi = 0_u16;
    for p in pixels {
        if *p < lo {
            lo = *p;
        }
        if *p > hi {
            hi = *p;
        }
    }
    println!("bayer min={} max={}", lo, hi);

    let rgba = demosaic(&raw, &pixels, black, white);

    let mut img = image::DynamicImage::new_rgba8(raw.width as u32, raw.height as u32);
    let buf = img.as_mut_rgba8().unwrap();
    for y in 0..raw.height {
        for x in 0..raw.width {
            let i = (y * raw.width + x) * 4;
            *buf.get_pixel_mut(x as u32, y as u32) =
                image::Rgba([rgba[i], rgba[i + 1], rgba[i + 2], 255]);
        }
    }

    img.save(output).unwrap();
    println!("wrote {}", output);
}

/// Minimal bilinear demosaic: for each output pixel, each RGB channel is the
/// average of the samples of that color within the 3x3 neighbourhood (the
/// classic 2/4-neighbour bilinear rule falls out of this on a 2x2 CFA).
fn demosaic(raw: &RawImage, bayer: &[u16], black: f32, white: f32) -> Vec<u8> {
    let w = raw.width;
    let h = raw.height;
    let span = white - black;
    let cfa = &raw.camera.cfa;
    let mut out = vec![0_u8; w * h * 4];

    for y in 0..h {
        for x in 0..w {
            let mut rgb = [0_f32; 3];
            for c in 0..3 {
                let mut sum = 0_f32;
                let mut n = 0_f32;
                for dy in -1..=1 {
                    let yy = y as i64 + dy;
                    if yy < 0 || yy >= h as i64 {
                        continue;
                    }
                    for dx in -1..=1 {
                        let xx = x as i64 + dx;
                        if xx < 0 || xx >= w as i64 {
                            continue;
                        }
                        if (cfa.color_at(yy as usize, xx as usize) as usize) == c {
                            sum += bayer[yy as usize * w + xx as usize] as f32;
                            n += 1.0;
                        }
                    }
                }
                let v = if n > 0.0 {
                    (sum / n - black) / span
                } else {
                    0.0
                };
                rgb[c] = if v < 0.0 {
                    0.0
                } else if v > 1.0 {
                    1.0
                } else {
                    v
                } * 255.0;
            }
            let i = (y * w + x) * 4;
            out[i] = rgb[0] as u8;
            out[i + 1] = rgb[1] as u8;
            out[i + 2] = rgb[2] as u8;
            out[i + 3] = 255;
        }
    }
    out
}
