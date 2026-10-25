#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod export;
mod histogram;
mod library;
mod model;
mod preview;
mod theme;
mod ui;
mod util;

use eframe::egui;

use app::EmulsionApp;

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([1000.0, 680.0])
            .with_icon(theme::window_icon()),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Emulsion",
        options,
        Box::new(|cc| Ok(Box::new(EmulsionApp::new(cc)))),
    );
    std::process::exit(0);
}
