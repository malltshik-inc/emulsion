use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn film_files(dir: &Path, category: &str, out: &mut Vec<(String, String, String)>) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    files.sort_by_key(|path| path.file_name().map(|n| n.to_ascii_lowercase()));

    for path in files {
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let relative = format!(
            "/assets/film/{}/{}",
            category,
            path.file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default()
        );
        out.push((name, category.to_owned(), relative));
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let root = manifest.join("assets").join("film");
    println!("cargo:rerun-if-changed={}", root.display());

    let mut films = Vec::new();
    film_files(&root.join("bw"), "bw", &mut films);
    film_files(&root.join("color"), "color", &mut films);

    let mut code = String::from("pub static FILMS: &[Film] = &[\n");
    for (name, category, relative) in &films {
        let variant = if category == "bw" {
            "BlackAndWhite"
        } else {
            "Color"
        };
        writeln!(
            code,
            "    Film {{ name: {:?}, category: Category::{}, png: include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {:?})) }},",
            name, variant, relative
        )
        .expect("write registry");
    }
    code.push_str("];\n");

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));
    std::fs::write(out.join("film_registry.rs"), code).expect("write film registry");
}
