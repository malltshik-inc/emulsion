use std::env;

use emulsion_core::export::{save_jpeg, save_png};
use emulsion_core::params::PipelineParams;

const USAGE: &str = "usage: develop <input> <output.png|output.jpg> [--invert]";

fn main() {
    let args = env::args().collect::<Vec<String>>();
    if args.len() < 3 {
        eprintln!("{}", USAGE);
        std::process::exit(2);
    }
    let input = &args[1];
    let output = &args[2];
    let invert = args.contains(&"--invert".into());

    let mut params = PipelineParams::default();
    params.invert = invert;

    match emulsion_core::process(input, &params) {
        Ok(img) => {
            let lower = output.to_lowercase();
            let result = if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
                save_jpeg(output, &img, 92)
            } else {
                save_png(output, &img)
            };
            match result {
                Ok(()) => println!("wrote {}: {}x{}", output, img.width, img.height),
                Err(e) => {
                    eprintln!("error: {}", e.message());
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("error: {}", e.message());
            std::process::exit(1);
        }
    }
}
