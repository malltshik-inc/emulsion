# Pre-release cleanup

Progress tracker for the v0.1.0 release pass. Each item is one commit (or a
small set). Tick off as done; `docs/TODO.md` travels with every commit.

## 1. Docs — remove stale behavior claims

- [x] ARCH: drop the fixed negative→positive conversion baseline (EV +1.4, WB +70, contrast +40, saturation +20) and `develop_defaults_apply_conversion_baseline`; the Output (`invert_sensitivity`) slider replaced it
- [x] ARCH: geometry order — the pipeline applies rotate_90 → fine angle → flip → crop (crop last)
- [x] ARCH + PRODUCT: sidecar `<name>.emulsion.json` is a backlog feature, not written on export
- [x] ARCH: drop hard test counts (kept numbers out of AGENTS + ARCH)
- [x] ARCH: preview caps 2000/1600, not 1800
- [x] ARCH: drop `set_zoom_factor(1.2)` claim
- [x] ARCH: status line lives in the stage; no bottom-panel status
- [x] ARCH: `take_available_space()` gotcha reframed (fixed-width panel here)
- [x] ARCH: `logo.png` sits at `assets/` root, not `assets/icons/`
- [x] PRODUCT: move Undo to a backlog section (required mechanism, UI later)
- [x] Cosmetic stragglers: process() signature, CollapsingHeader mention, installer formats, export pre-fill `.jpg`, thumbnail fraction, "Open" button label, §4 preview-pane wording

## 2. Dead code

- [x] core `error.rs`: remove unused `ok`/`err` helpers
- [x] core `colorops::invert` unused outside tests → remove (and its tests)
- [x] core `tonemap_and_quantize` wrapper unused outside tests → remove or fold
- [x] core `LinearImage::px` dup of `pixel()` → remove, rename test call sites
- [x] core `lib.rs` unused root re-exports
- [x] app: `EmulsionApp.folder` write-only field → remove
- [x] keep `save_sidecar` (backlog feature, see docs), keep its test

## 3. Tests

- [x] core: `save_png`, `process()`, `decode` on a regular image, `CoreError::message`
- [x] app: `histogram::make_histogram`
- [x] app: `util::aspect_label`, `util::cover_uv`
- [ ] app: crop drag math (extracted in item 6) — `free_resize`, `ratio_resize`, `ratio_draw`, `crop_from_drag`, `normalize_crop`
- [x] app: sparse (not-yet-loaded) index handling in `sync_thumbnails` if touched

## 4. Perf — core

- [x] `develop` / invert path: reuse a buffer instead of cloning the whole f32 base per call (`develop_into`, app holds scratch buffers)
- [x] `invert_negative`: replace per-pixel `powf` with an sRGB LUT (shared `transfer` module)
- [x] `demosaic_bilinear`: precompute per-phase neighbor masks, drop the per-pixel `color_at`
- [x] `apply_black_point`: hoist per-channel constants out of the loop
- [x] `film::lut`: keep the lock across decode (no duplicate concurrent decodes)
- [x] share `srgb_encode`/`srgb_decode` + the RGBA→linear loop between `decode.rs` and `tonemap.rs` (`transfer.rs`)

## 5. Perf — app

- [ ] sidebar rows: layout the galley once and paint it (no measure-then-remake, no extra `String`)
- [ ] `paint_stage_background`: cache the mesh/dots, rebuild only on resize
- [ ] `sync_thumbnails`: queue dirty rows instead of sweeping the whole list per thumbnail

## 6. Structure — app

- [ ] extract pure crop drag/resize math out of `ui/crop.rs` (+ tests)
- [ ] dedupe ghost-reset / outline-button builders across settings/film/crop
- [ ] dedupe MP computation; `library.rs` uses `util::file_extension`
- [ ] dedupe "Open a folder to start" string
- [ ] `stage.rs`: single Space-recenter path