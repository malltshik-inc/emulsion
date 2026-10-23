use std::env;

use emulsion_core::export::{save_jpeg, save_png};
use emulsion_core::params::PipelineParams;

const USAGE: &str = "usage: develop <input> <output.png|output.jpg> [--invert] [--ev N] [--contrast N] [--saturation N] [--film ID]";

fn main() {
    let args = env::args().collect::<Vec<String>>();
    if args.len() < 3 {
        eprintln!("{}", USAGE);
        std::process::exit(2);
    }
    let input = &args[1];
    let output = &args[2];

    let mut params = PipelineParams::default();
    let mut rest = args[3..].iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--invert" => params.invert = true,
            "--ev" => {
                if let Some(v) = rest.next().and_then(|v| v.parse().ok()) {
                    params.exposure = v;
                }
            }
            "--contrast" => {
                if let Some(v) = rest.next().and_then(|v| v.parse().ok()) {
                    params.contrast = v;
                }
            }
            "--saturation" => {
                if let Some(v) = rest.next().and_then(|v| v.parse().ok()) {
                    params.saturation = v;
                }
            }
            "--film" => {
                let Some(id) = rest.next().and_then(|v| v.parse::<u16>().ok()) else {
                    eprintln!("--film needs a simulation id");
                    std::process::exit(2);
                };
                let Some(film) = emulsion_core::film::find(id) else {
                    eprintln!("no film simulation with id {}", id);
                    std::process::exit(2);
                };
                eprintln!("film: {} ({})", film.name, id);
                params.film = Some(id);
            }
            other => {
                eprintln!("unknown argument: {}", other);
                std::process::exit(2);
            }
        }
    }

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
