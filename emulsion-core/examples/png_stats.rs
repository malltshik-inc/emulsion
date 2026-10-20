use std::env;

use image::GenericImageView;

const USAGE: &str = "usage: png_stats <image.png>";

fn main() {
    let args = env::args().collect::<Vec<String>>();
    if args.len() != 2 {
        eprintln!("{}", USAGE);
        std::process::exit(2);
    }
    let img = image::ImageReader::open(&args[1])
        .unwrap()
        .decode()
        .unwrap();
    let Some(rgb) = img.as_rgba8() else {
        eprintln!("image is not rgba8");
        std::process::exit(1);
    };
    let dim = img.dimensions();
    println!("size: {}x{}", dim.0, dim.1);

    let mut sum = [0_u64; 4];
    let mut min = [255_u8; 4];
    let mut max = [0_u8; 4];
    for y in 0..dim.1 {
        for x in 0..dim.0 {
            let p = rgb.get_pixel(x, y);
            for c in 0..4 {
                let v = p.0[c];
                sum[c] += v as u64;
                if v < min[c] {
                    min[c] = v;
                }
                if v > max[c] {
                    max[c] = v;
                }
            }
        }
    }
    let n = dim.0 as u64 * dim.1 as u64;
    println!("channels r,g,b,a:");
    for c in 0..4 {
        println!("  ch{}: mean={}", c, sum[c] / n);
    }
    println!("min rgb: [{}, {}, {}]", min[0], min[1], min[2]);
    println!("max rgb: [{}, {}, {}]", max[0], max[1], max[2]);
}
