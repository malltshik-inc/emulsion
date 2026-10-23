# Emulsion

Desktop photo editor that converts negative film scans into positive images.
Simple UI, precise color, "a-ha colors". Windows first; macOS/Linux later.

## Run it

```
cargo build --release
target\release\emulsion-app.exe
```

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