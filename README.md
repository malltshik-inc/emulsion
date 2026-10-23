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
- Film simulations: searchable strip of bundled colour + B&W Hald CLUTs
  (RawTherapee collection; attribution in `emulsion-core/assets/film/`)
- Five sliders (white balance, tint, EV, contrast, saturation), invert, rotate 90° + fine angle
- Double-click a slider to reset it
- Wheel-zoom, drag-pan, Space to recenter.
- **Export…** → full-res JPEG/PNG named after the source, plus a
  `<name>.emulsion.json` sidecar with the exact settings used (originals never touched)

CLI for testing/batch: `target\release\develop.exe <in.orf> <out.png> [--invert]`

## Who's inside

- `emulsion-core` — pure Rust pipeline (rawler demosaic + color pipeline, unit-tested)
- `emulsion-app` — egui frontend

Product spec: `docs/PRODUCT.md` · Technical deep-dive: `docs/ARCH.md`