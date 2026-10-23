# Emulsion

Desktop photo editor that converts negative film scans into positive images.
Simple UI, precise color. Windows, macOS and Linux.

## Install

Prebuilt packages are attached to each [release](https://github.com/malltshik/emulsion/releases/latest):

- **Windows** — `.msi` installer (double-click).
- **macOS** — `.dmg` (open, drag Emulsion to Applications). The build is unsigned,
  so the first launch needs right-click → **Open**, or:
  `xattr -dr com.apple.quarantine /Applications/Emulsion.app`
- **Linux** — `.deb` (Debian/Ubuntu).

Linux one-liner (replace the file name with the one shown on the Releases page):

```sh
curl -fsSL -o emulsion.deb \
  https://github.com/malltshik/emulsion/releases/latest/download/emulsion_0.1.0_amd64.deb \
  && sudo apt install -y ./emulsion.deb
```

## Build from source

Needs a recent stable Rust (edition 2024). On Debian/Ubuntu install the GUI deps first:

```sh
sudo apt install libxkbcommon-dev libwayland-dev libxcb-randr0-dev \
  libxcb-xfixes0-dev libxcb-shape0-dev libxcb-xkb-dev libgl1-mesa-dev libgtk-3-dev
```

```sh
cargo build --release -p emulsion-app
target/release/emulsion-app        # target\release\emulsion-app.exe on Windows
```

Everything the app needs is compiled in — fonts (Ubuntu, bundled under
`emulsion-app/assets/fonts/`) and the film LUTs (`emulsion-core/assets/film/`).

Build installers locally with [cargo-packager](https://github.com/crabnebula-dev/cargo-packager):
`cd emulsion-app && cargo packager --release` (produces `.msi` / `.dmg` / `.deb`).

## Features

- **Open folder…** → all images in it (recursive)
- Click an image → live preview; each file keeps its own settings
- **Develop** (top strip) flips negative→positive with a built-in baseline
  (EV +1.4, WB +80, contrast +82, saturation +20) so a neutral scan converts well
- **Simulations** tab: searchable strip of bundled colour + B&W Hald CLUTs
  (RawTherapee collection; attribution in `emulsion-core/assets/film/`)
- Sliders: white balance, tint, EV, contrast, shadows, highlights, saturation, vignette;
  rotate 90° + fine angle + flip horizontal
- Copy/Paste buttons apply one photo's settings to others
- Double-click a slider to reset it
- Wheel-zoom, drag-pan, Space to recenter.
- **Export…** → dialog: *This photo* (full-res JPEG/PNG named after the source) or
  *All edited photos* (PNG batch to a folder); originals never touched

CLI for testing/batch: `target\release\develop.exe <in.orf> <out.png> [--invert]`

## Who's inside

- `emulsion-core` — pure Rust pipeline (rawler demosaic + color pipeline, unit-tested)
- `emulsion-app` — egui frontend

Product spec: `docs/PRODUCT.md` · Technical deep-dive: `docs/ARCH.md`