#[cfg(windows)]
fn main() {
    println!("cargo:rerun-if-changed=assets/icons/icon.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icons/icon.ico");
    res.set("ProductName", "Emulsion");
    res.set("FileDescription", "Emulsion");
    res.compile().expect("compile Windows resources");
}

#[cfg(not(windows))]
fn main() {}
